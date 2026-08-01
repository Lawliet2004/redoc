import { afterEach, beforeEach } from "vitest";

beforeEach(() => {
  document.documentElement.lang = "en";
  document.title = "Redoc";
  document.body.replaceChildren();
});

afterEach(() => {
  document.body.replaceChildren();
});
