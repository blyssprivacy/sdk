export function isNode(): boolean {
  return !!(
    typeof process !== "undefined" &&
    process.versions &&
    process.versions.node
  );
}
