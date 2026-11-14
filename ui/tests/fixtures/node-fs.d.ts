// The one Node function the bridge tests use to read a binary fixture.
// (@types/node is not installed; this keeps the tests type-checked without it.)
declare module "node:fs" {
  export function readFileSync(path: string): Uint8Array;
}
