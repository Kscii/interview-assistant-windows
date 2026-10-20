import { test, expect } from "@playwright/test";
test("first-run setup, profile editing and review", async ({ page }) => {
  await page.goto("/?preview=1");
  await expect(page.getByText("界面预览 · 不调用模型")).toBeVisible();
  await expect(
    page.getByRole("button", { name: "识别并回答", exact: true }),
  ).toBeDisabled();
  await page.getByRole("button", { name: "面试档案", exact: true }).click();
  await page.getByRole("button", { name: "创建第一套档案" }).click();
  await page
    .getByLabel("档案名称", { exact: true })
    .fill("Fictional software interview");
  await page
    .getByLabel("面试目标", { exact: true })
    .fill("Graduate software engineer");
  await page
    .getByLabel("个人素材正文")
    .fill(
      "I study software engineering. I built a small library website with classmates.",
    );
  await page.getByRole("button", { name: "加入资料", exact: true }).click();
  await page.getByRole("button", { name: "保存草稿", exact: true }).click();
  await expect(page.getByText("档案草稿已保存")).toBeVisible();
  await page.getByRole("button", { name: "回答要求", exact: true }).click();
  await page.getByLabel("目标朗读时长（秒）").fill("90");
  await page.getByRole("button", { name: "保存草稿", exact: true }).click();
  await page.getByRole("button", { name: "整理预览", exact: true }).click();
  await page
    .getByLabel("个人定位", { exact: true })
    .fill("A software engineering student who enjoys practical projects.");
  await page.getByRole("button", { name: "确认启用", exact: true }).click();
  await expect(page.getByText("档案已确认并启用")).toBeVisible();
  await page.getByRole("button", { name: "使用首页", exact: true }).click();
  await expect(
    page.getByRole("heading", { name: "Fictional software interview" }),
  ).toBeVisible();
  await expect(page.getByText(/90 秒回答/)).toBeVisible();
  await page.screenshot({ path: "artifacts/home-preview.png", fullPage: true });
});
test("model setup and selected roles", async ({ page }) => {
  await page.goto("/?preview=1");
  await page.getByRole("button", { name: "模型连接", exact: true }).click();
  await page.getByRole("button", { name: "添加连接", exact: true }).click();
  await page.getByLabel("连接名称", { exact: true }).fill("Example model");
  await page.getByLabel("模型 ID", { exact: true }).fill("example-model");
  await page
    .getByLabel("API Key", { exact: true })
    .fill("preview-only-not-a-real-key");
  await page.getByRole("button", { name: "保存连接", exact: true }).click();
  await expect(page.getByText("已保存密钥")).toBeVisible();
  await page
    .getByLabel("资料整理与准备答案模型")
    .selectOption({ label: "Example model · example-model" });
  await page.getByRole("radio").check();
  await page.getByRole("button", { name: "保存模型分配", exact: true }).click();
  await expect(page.getByText("模型分配已保存")).toBeVisible();
});
test("settings retain configured overlay and shortcuts", async ({ page }) => {
  await page.goto("/?preview=1");
  await page.getByRole("button", { name: "屏幕与快捷键", exact: true }).click();
  await page.getByLabel("字号", { exact: true }).fill("32");
  await page.getByLabel("识别并回答", { exact: true }).fill("Ctrl+Alt+Q");
  await page.getByRole("button", { name: "保存设置", exact: true }).click();
  await page.getByRole("button", { name: "使用首页", exact: true }).click();
  await expect(page.getByText("Ctrl+Alt+Q", { exact: true })).toBeVisible();
});

test("selector stays transparent and accepts a visible drag region", async ({
  page,
}) => {
  await page.goto("/?preview=1&window=selector");
  await expect(page.locator("html")).toHaveCSS(
    "background-color",
    "rgba(0, 0, 0, 0)",
  );
  await page.mouse.move(200, 200);
  await page.mouse.down();
  await page.mouse.move(800, 350);
  await page.mouse.up();
  await expect(page.locator(".selection-box span")).toHaveText("600 × 150");
  await expect(page.locator(".selection-hint p")).toHaveCount(0);
});

test("new caption resets scroll and capture errors remain visible", async ({
  page,
}) => {
  await page.goto("/?preview=1&window=overlay");
  await expect(page.getByText("按识别快捷键读取问题")).toBeVisible();
  const caption = {
    request_id: "first",
    question: "First question",
    answer: "A long answer.\n".repeat(100),
    profile_id: "example",
    revision: "1",
    status: "ready",
    detail: "",
    elapsed_ms: 900,
    cached: false,
  };
  await page.evaluate(
    (c) => window.dispatchEvent(new CustomEvent("caption", { detail: c })),
    caption,
  );
  await page.locator(".caption-content").evaluate((el) => {
    el.scrollTop = 800;
  });
  await page.evaluate(() =>
    window.dispatchEvent(
      new CustomEvent("notice", { detail: "没有识别到文字，请重新框选" }),
    ),
  );
  await expect(page.getByText("没有识别到文字，请重新框选")).toBeVisible();
  await page.evaluate(
    (c) => window.dispatchEvent(new CustomEvent("caption", { detail: c })),
    {
      ...caption,
      request_id: "second",
      question: "Second question",
      answer: "A complete new answer.",
    },
  );
  await expect(page.getByText("Second question")).toBeVisible();
  await expect(page.getByText("A complete new answer.")).toBeVisible();
  await expect(page.locator(".caption-notice")).toHaveCount(0);
  await expect
    .poll(() => page.locator(".caption-content").evaluate((el) => el.scrollTop))
    .toBe(0);
});
