export function parse(input: string): number {
  // @ts-expect-error legacy input
  // eslint-disable-next-line no-restricted-globals
  return Number(input);
}
