/**
 * 主题偏好查询。
 *
 * CSS 侧的深浅主题由 tokens.css 经 prefers-color-scheme 自动切换；
 * 少数无法走 CSS 的场景（如原生窗口 resize 守卫背景色）从这里取值。
 * html[data-theme] 手动覆盖接入后，这里需一并读取该属性。
 */
export function prefersDarkColorScheme(): boolean {
  if (typeof window === "undefined" || typeof window.matchMedia !== "function") return false;
  return window.matchMedia("(prefers-color-scheme: dark)").matches;
}
