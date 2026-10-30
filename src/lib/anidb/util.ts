/*
 * Small helpers for building requests and decoding coded values
 */

/** request parameters for a command that takes either an id or a name */
export function nameOrId(
  param: string | number,
  idField: string,
  nameField: string,
  otherFields?: Record<string, string | number>,
): Record<string, string | number> {
  const request: Record<string, string | number> = { ...otherFields };
  if (typeof param === "string") {
    request[nameField] = param;
  } else {
    request[idField] = param;
  }
  return request;
}

/** swaps keys and values: `{ a: 1 }` becomes `{ 1: "a" }` */
export function reverseObj<K extends string, V extends string | number>(
  input: Record<K, V>,
): Record<V, K> {
  const reversed = {} as Record<V, K>;
  for (const [key, value] of Object.entries(input) as [K, V][]) {
    reversed[value] = key;
  }
  return reversed;
}

// vim: tabstop=2 shiftwidth=2 softtabstop=0 smarttab expandtab
