# Interview Assistant · Windows

Windows 上的个人面试准备与字幕工具。提前整理个人资料，按快捷键读取指定屏幕区域，再用自己的模型 API 生成回答。

独立项目，MIT 许可证。只使用屏幕 OCR，不安装浏览器扩展，不注入网页，不读取浏览器请求。

## 使用流程

1. 在 **模型连接** 中填写服务商、模型 ID、API Key。保存后可以测试连接；这个按钮会产生一次真实 API 调用。分别指定“资料整理模型”和“即时回答模型”。
2. 新建 **面试档案**，放入简历、经历素材和回答偏好。支持 TXT、Markdown、DOCX、PDF、PNG、JPEG、WebP；可以直接粘贴长文本。
3. 设置回答语言、朗读时长、语速、表达风格和补充要求。默认 60 秒、90 词/分钟，正文后用 `|` 分隔两句可选补充。
4. 点击 **开始整理资料**。逐文件读取、归纳并保存检查点；在预览中集中回答补充问题，也可跳过。核对、编辑后点击 **确认启用**。
5. 在 **准备答案** 中添加题目或让模型建议题目。批量生成的答案立即启用，可逐字修改、停用或删除。
6. 在 **屏幕与快捷键** 中框选问题区域。区域会保存；显示器位置、分辨率或缩放改变后，需要重新框选。
7. 按 **Ctrl+Alt+Space** 识别并回答。按 **Ctrl+Alt+H** 显示或隐藏字幕，后台任务不受影响。两个快捷键都可修改。

字幕默认在屏幕上方，白底黑字，显示识别到的问题及完整回答。默认鼠标穿透；通过托盘或设置页进入调整模式后，可移动、缩放、滚动查看。完成调整恢复鼠标穿透。截图前会暂时移开字幕，避免识别到自身内容。

关闭设置窗口后仍在系统托盘运行；从托盘菜单选择“退出”才会关闭程序。

## 可靠性约定

- 大小写、常见标点和空白变化不影响同题命中；数字、否定和技术符号保留。
- 同题正在生成时不重复请求；失败后再次触发会重试。命中缓存时原样返回。需要新内容时使用“重新生成”。
- 准备答案优先于现场缓存。相近问题只借鉴相关素材，仍单独作答。
- 新问题立即取代旧问题；旧任务的迟到结果不能覆盖新问题。
- 默认单模型；可选最多 5 个连接竞速。第一个有效完整回答胜出，后来的回答不替换。
- **5 秒是响应目标，不是网络保证。** 5 秒后提示仍在等待，并继续接受有效结果；最终等待上限默认 60 秒，可设置为 10–300 秒。所有模型失败时明确显示失败，不冒充已生成回答。
- 当前页面显示的耗时是回答阶段耗时，不包含截图、OCR 和屏幕绘制。
- 草稿不影响已启用版本。修改档案或回答规则并重新启用后，旧版本答案仍可查看，但不会误命中；可以按新档案重新生成。

## 模型与资料

支持 OpenAI Responses、OpenAI 兼容 Chat Completions、Gemini 和 Anthropic 原生接口，实际通过 Rig 调用。模型 ID 由用户填写，避免把变化中的模型名单固化在程序中。

连接的“高级参数”使用该接口原生 JSON，例如推理等级、服务等级等。是否支持取决于具体模型及账户；程序不会替你申请权限或保证某种等级更快。不要把密钥填写在高级参数中。

图片和扫描 PDF 在资料预处理时发送给支持相应输入的模型；需要勾选该连接的图片/PDF 能力。DOCX 在本地提取文本和表格段落。旧式 `.doc` 请先另存为 `.docx`。一次导入的单文件不超过 18 MB。

现场截图默认只在本地 CPU 识别。识别失败可重试、重新框选或手动输入；**云端重新识别选区** 会把最近一次选区截图发送给已配置的资料整理模型。截图仅暂存在进程内存，不落盘。

## 数据与导出

- 资料、配置、答案和任务状态保存在当前 Windows 用户的 `%LOCALAPPDATA%\dev.kscii.interview-assistant\`。
- API Key 保存在 Windows 凭据管理器中；界面只能看到“已保存”，不会读回密钥。
- 档案导出包含资料原文、整理结果和准备答案，**不包含 API Key 或模型连接**。导出文件本身包含个人资料，请自行保管。
- 没有内置公共 API Key，不读取其他项目密钥。示例资料全部虚构。
- 本地 OCR 无需联网；调用模型时，选定的个人素材与问题会发送到你配置的服务商。

## 安装与系统要求

目标系统为 Windows 10 2004（build 19041）及以后 / Windows 11，x64 CPU，无需 GPU。

安装包包含 PP-OCRv4 检测/识别模型、方向模型、字典、ONNX Runtime、Visual C++ 运行库和第三方许可证。如果电脑没有 WebView2，安装程序会联网安装它。

从 GitHub Actions 的 **Windows build and checks** 成功构建中下载 `Interview-Assistant-windows-x64`，解压并运行其中的 `.exe`。当前构建未配置代码签名。

## 本地开发与打包

Windows 构建依赖：Rust stable（至少 1.95）、Node.js 22+、Python 3.12+、Visual Studio C++ Build Tools、Windows SDK，以及 Tauri 2 的 Windows 构建环境。

```powershell
npm ci
python scripts/prepare-resources.py
./scripts/prepare-msvc.ps1
python scripts/collect-licenses.py
npm run tauri -- dev
```

资源下载地址及 SHA-256 固定在 `scripts/resources.lock.json` 中；构建时校验。Python 仅用于构建，最终应用不依赖 Python。

```powershell
cargo fmt --all -- --check
cargo test -p interview-core --locked
cargo test -p interview-assistant --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
npm run build
npm run tauri -- build --bundles nsis
./scripts/test-installer.ps1
```

安装包输出：`target/release/bundle/nsis/`。安装测试会静默安装到项目 `artifacts/installed/`，验证文件并从安装目录执行 CPU OCR，结果保存在 `artifacts/installed-ocr.json`。这项测试不调用模型，也不创建个人档案。

仅预览界面：

```sh
npm run dev
# 浏览器打开 http://127.0.0.1:1420/?preview=1
```

此模式明确标为“界面预览”，使用临时虚构状态，不连接模型，也不能证明 Windows 截图/快捷键可用。

```sh
npx playwright install chromium
npm run test:e2e
```

架构说明见 [docs/architecture.md](docs/architecture.md)，Windows 实机验收见 [docs/acceptance.md](docs/acceptance.md)。
