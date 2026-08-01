import { en } from "./en";

type Dictionary = typeof en;

type Leaves<T> = T extends object
  ? {
      [K in keyof T & string]: T[K] extends string
        ? `${K}`
        : `${K}.${Leaves<T[K]>}`;
    }[keyof T & string]
  : never;

export type TranslationKey = Leaves<Dictionary>;

export function t(key: TranslationKey | (string & {}), params?: Record<string, any>): string {
  const keys = key.split(".");
  let current: any = en;

  for (const k of keys) {
    if (current && typeof current === "object" && k in current) {
      current = current[k];
    } else {
      return key;
    }
  }

  if (typeof current !== "string") {
    return key;
  }

  let result: string = current;
  if (params) {
    for (const [p, val] of Object.entries(params)) {
      result = result.replace(new RegExp(`\\{${p}\\}`, "g"), String(val));
    }
  }

  return result;
}

export * from "./en";
