// 002-multi-platform-publish：图床上传服务（GitHub 仓库图床）。
//
// 设计来源：移植 doocs/md `services/upload`（providers.ts 的 ghFileUpload）+ `useImageUploader`
// 的「插入即上传、MD5 去重缓存」思路。职责：把文章 Markdown 里的【本地 assets/ 图片】先传到
// GitHub 图床换成【公网外链】，使后续各平台（知乎/掘金等）粘贴 Markdown 时由平台自动转存——
// 这正是 doocs/md + cose 的原始链路，无需为每个平台单独维护图片上传端点。
//
// 纯前端实现：本地图片字节经 Tauri `read_asset_data_url` 命令读取（已做防目录穿越校验），
// GitHub Contents API 支持 CORS，可在 WebView 直接 PUT。配置存浏览器 localStorage。

import { api } from "@/bindings/commands";

const CONFIG_KEY = "imageHost:github";

/** GitHub 仓库图床配置（对齐 doocs/md githubConfig 字段）。 */
export interface GithubImageHostConfig {
  /** `username/repo`，也兼容粘贴完整仓库 URL。 */
  repo: string;
  /** 分支，默认 main。 */
  branch: string;
  /** Personal Access Token（需 repo / contents 写权限）。 */
  accessToken: string;
  /** 是否把 raw.githubusercontent.com 链接替换为 jsDelivr CDN。 */
  useCDN: boolean;
}

/** 读取已保存的图床配置；未配置或不完整返回 null。 */
export function getGithubConfig(): GithubImageHostConfig | null {
  try {
    const raw = localStorage.getItem(CONFIG_KEY);
    if (!raw) return null;
    const cfg = JSON.parse(raw) as Partial<GithubImageHostConfig>;
    if (!cfg.repo || !cfg.accessToken) return null;
    return {
      repo: cfg.repo,
      branch: cfg.branch || "main",
      accessToken: cfg.accessToken,
      useCDN: cfg.useCDN ?? false,
    };
  } catch {
    return null;
  }
}

export function saveGithubConfig(cfg: GithubImageHostConfig): void {
  localStorage.setItem(CONFIG_KEY, JSON.stringify(cfg));
}

/** 从 `username/repo` 或完整仓库 URL 解析出 `{ owner, repo }`。 */
function parseRepo(input: string): { owner: string; repo: string } {
  const cleaned = input
    .trim()
    .replace(/^https?:\/\/github\.com\//i, "")
    .replace(/\.git$/i, "")
    .replace(/^\/+|\/+$/g, "");
  const [owner, repo] = cleaned.split("/");
  if (!owner || !repo) {
    throw new Error(`GitHub 仓库格式应为 username/repo，当前：${input}`);
  }
  return { owner, repo };
}

/** `年/月/日` 目录（对齐 doocs/md getDir，便于在仓库内归档）。 */
function getDir(): string {
  const d = new Date();
  const mm = String(d.getMonth() + 1).padStart(2, "0");
  const dd = String(d.getDate()).padStart(2, "0");
  return `${d.getFullYear()}/${mm}/${dd}`;
}

/** `时间戳-uuid.扩展名`（对齐 doocs/md getDateFilename，避免重名覆盖）。 */
function dateFilename(filename: string): string {
  const ext = filename.includes(".") ? filename.split(".").pop() : "png";
  const uuid =
    typeof crypto !== "undefined" && "randomUUID" in crypto
      ? crypto.randomUUID()
      : Math.random().toString(36).slice(2);
  return `${Date.now()}-${uuid}.${ext}`;
}

/** SHA-256（hex）作为去重缓存键：同图不重传（对齐 doocs/md 的 MD5 去重，改用原生 SubtleCrypto 免依赖）。 */
async function sha256Hex(input: string): Promise<string> {
  const bytes = new TextEncoder().encode(input);
  const digest = await crypto.subtle.digest("SHA-256", bytes);
  return Array.from(new Uint8Array(digest))
    .map((b) => b.toString(16).padStart(2, "0"))
    .join("");
}

const HASH_CACHE_KEY = "imageHost:uploadedMap";

function getCache(): Record<string, string> {
  try {
    return JSON.parse(localStorage.getItem(HASH_CACHE_KEY) || "{}");
  } catch {
    return {};
  }
}

function setCache(hash: string, url: string): void {
  const map = getCache();
  map[hash] = url;
  localStorage.setItem(HASH_CACHE_KEY, JSON.stringify(map));
}

/** 把单张图片的 base64 内容 PUT 到 GitHub 仓库，返回公网外链（对齐 doocs/md ghFileUpload）。 */
async function uploadToGithub(
  base64: string,
  filename: string,
  cfg: GithubImageHostConfig,
): Promise<string> {
  const { owner, repo } = parseRepo(cfg.repo);
  const branch = cfg.branch || "main";
  const path = `${getDir()}/${dateFilename(filename)}`;
  const url = `https://api.github.com/repos/${owner}/${repo}/contents/${path}`;

  const res = await fetch(url, {
    method: "PUT",
    headers: {
      Authorization: `token ${cfg.accessToken}`,
      Accept: "application/vnd.github+json",
    },
    body: JSON.stringify({
      content: base64,
      branch,
      message: `upload by quick-publish`,
    }),
  });

  if (!res.ok) {
    const text = await res.text();
    throw new Error(`GitHub 图床上传失败（HTTP ${res.status}）：${text.slice(0, 160)}`);
  }
  const json = (await res.json()) as { content?: { download_url?: string } };
  const downloadUrl = json.content?.download_url;
  if (!downloadUrl) {
    throw new Error("GitHub 图床返回缺少 download_url");
  }
  if (cfg.useCDN) {
    return downloadUrl.replace(
      `raw.githubusercontent.com/${owner}/${repo}/${branch}/`,
      `fastly.jsdelivr.net/gh/${owner}/${repo}@${branch}/`,
    );
  }
  return downloadUrl;
}

/** 判定 Markdown/HTML 里的图片引用是否为「需上传的本地相对路径」（排除外链/data/blob）。 */
function isLocalRef(ref: string): boolean {
  return !/^(https?:)?\/\//i.test(ref) && !/^(data|blob):/i.test(ref);
}

/** 收集 Markdown 正文中所有本地图片引用路径（Markdown `![]()` 与内联 `<img src>`，去重保序）。 */
function collectLocalImageRefs(markdown: string): string[] {
  const refs: string[] = [];
  const push = (r: string | undefined) => {
    if (!r) return;
    const ref = r.trim().replace(/^<|>$/g, "");
    if (isLocalRef(ref) && !refs.includes(ref)) refs.push(ref);
  };
  // Markdown 图片：![alt](path "title")
  const mdImg = /!\[[^\]]*\]\(\s*([^)\s]+)(?:\s+["'][^"']*["'])?\s*\)/g;
  for (const m of markdown.matchAll(mdImg)) push(m[1]);
  // 内联 HTML：<img ... src="path">
  const htmlImg = /<img[^>]+src=["']([^"']+)["']/gi;
  for (const m of markdown.matchAll(htmlImg)) push(m[1]);
  return refs;
}

/** 字面量全局替换（避免把 ref 当正则；同图多处引用一并替换）。 */
function replaceAllLiteral(haystack: string, needle: string, replacement: string): string {
  return haystack.split(needle).join(replacement);
}

/** 上传单个本地图片（含去重缓存）：读字节 → base64 → 传 GitHub → 返回外链。 */
async function uploadLocalRef(
  ref: string,
  cfg: GithubImageHostConfig,
): Promise<string> {
  const dataUrl = await api.readAssetDataUrl(ref);
  const base64 = dataUrl.split(",").pop() ?? "";
  if (!base64) throw new Error(`本地图片读取为空：${ref}`);

  const hash = await sha256Hex(base64);
  const cached = getCache()[hash];
  if (cached) return cached;

  const filename = ref.split("/").pop() || "image.png";
  const url = await uploadToGithub(base64, filename, cfg);
  setCache(hash, url);
  return url;
}

/**
 * 把 Markdown 中所有本地图片替换为 GitHub 图床外链。无本地图片时原样返回；
 * 未配置图床但存在本地图片时抛错（提示去配置）。任一图片上传失败即整体抛错
 * （对齐后端「图片全有或全无」约束 FR-010a），避免残留本地路径导致平台端坏图。
 */
export async function externalizeLocalImages(markdown: string): Promise<string> {
  const refs = collectLocalImageRefs(markdown);
  if (refs.length === 0) return markdown;

  const cfg = getGithubConfig();
  if (!cfg) {
    throw new Error(
      "文章包含本地图片，但尚未配置 GitHub 图床。请先在「图床设置」中填写仓库与 Token。",
    );
  }

  let out = markdown;
  for (const ref of refs) {
    const url = await uploadLocalRef(ref, cfg);
    out = replaceAllLiteral(out, ref, url);
  }
  return out;
}
