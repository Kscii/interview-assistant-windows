import {
  useCallback,
  useEffect,
  useState,
  useId,
  cloneElement,
  type ReactNode,
  type ReactElement,
} from "react";
import {
  LayoutDashboard,
  UserRound,
  MessageSquareText,
  PlugZap,
  ScanLine,
  Plus,
  ArrowRight,
  Check,
  Keyboard,
  Eye,
  Play,
  FileText,
  LoaderCircle,
  Upload,
  Download,
  Trash2,
  Settings2,
  Monitor,
  BookOpen,
} from "lucide-react";
import { rpc, subscribe, pickFile, saveFile, preview } from "./api";
import type {
  Answer,
  Caption,
  Compiled,
  Connection,
  Job,
  Profile,
  Rules,
  Settings,
  Snapshot,
} from "./types";
import { emptyCompiled } from "./types";

type Page = "home" | "profiles" | "answers" | "models" | "screen";
type Run = (
  action: string,
  payload?: Record<string, unknown>,
  success?: string,
) => Promise<unknown>;
const nav = [
  ["home", "使用首页", LayoutDashboard],
  ["profiles", "面试档案", UserRound],
  ["answers", "准备答案", MessageSquareText],
  ["models", "模型连接", PlugZap],
  ["screen", "屏幕与快捷键", ScanLine],
] as const;
const labels = {
  home: ["准备充分，表达自然", "把准备留在这里，把注意力留给问题。"],
  profiles: ["你的经历，你的表达", "整理个人素材，准备适合自己的回答方式。"],
  answers: ["把常见问题提前准备好", "生成后立即可用，也可以逐字调整。"],
  models: ["选择适合每个阶段的模型", "资料整理重理解，即时回答重速度。"],
  screen: ["让工具适应你的屏幕", "框选一次，之后用快捷键直接读取。"],
};
export function Button({
  children,
  onClick,
  secondary = false,
  disabled = false,
  ...rest
}: {
  children: ReactNode;
  onClick?: () => void;
  secondary?: boolean;
  disabled?: boolean;
  type?: "button" | "submit";
  title?: string;
}) {
  return (
    <button
      className={secondary ? "button secondary" : "button"}
      onClick={onClick}
      disabled={disabled}
      {...rest}
    >
      {children}
    </button>
  );
}
function Field({
  label,
  children,
  hint,
}: {
  label: string;
  children: ReactNode;
  hint?: string;
}) {
  const id = useId();
  return (
    <div className="field">
      <label htmlFor={id}>{label}</label>
      {cloneElement(
        children as ReactElement<{ id: string; "aria-describedby"?: string }>,
        { id, "aria-describedby": hint ? `${id}-hint` : undefined },
      )}
      {hint && <small id={`${id}-hint`}>{hint}</small>}
    </div>
  );
}
function Empty({
  title,
  text,
  children,
}: {
  title: string;
  text: string;
  children?: ReactNode;
}) {
  return (
    <div className="empty">
      <BookOpen size={32} />
      <h3>{title}</h3>
      <p>{text}</p>
      {children}
    </div>
  );
}
const uid = () => crypto.randomUUID();
export default function App() {
  const [page, setPage] = useState<Page>("home"),
    [data, setData] = useState<Snapshot | null>(null),
    [selected, setSelected] = useState(""),
    [notice, setNotice] = useState(""),
    [busy, setBusy] = useState("");
  const refresh = useCallback(async () => {
    const s = await rpc<Snapshot>("snapshot");
    setData(s);
    setSelected((prev) =>
      s.profiles.some((p) => p.id === prev)
        ? prev
        : (s.settings.active_profile ?? s.profiles[0]?.id ?? ""),
    );
  }, []);
  useEffect(() => {
    void refresh().catch((e) => setNotice(String(e)));
  }, [refresh]);
  useEffect(() => {
    const subscriptions = [
      subscribe<Caption>("caption", (caption) =>
        setData((s) => (s ? { ...s, caption } : s)),
      ),
      subscribe<string>("notice", setNotice),
      subscribe<Job>("job", (job) => {
        setData((s) =>
          s
            ? {
                ...s,
                jobs: [
                  ...s.jobs.filter((j) => j.profile_id !== job.profile_id),
                  job,
                ],
              }
            : s,
        );
        if (job.status !== "running") void refresh();
      }),
      subscribe("settings-changed", () => void refresh()),
      subscribe<boolean>("visibility", (visible) =>
        setData((s) => (s ? { ...s, visible } : s)),
      ),
    ];
    return () => {
      subscriptions.forEach((s) => void s.then((f) => f()));
    };
  }, [refresh]);
  const run: Run = async (action, payload = {}, success) => {
    setBusy(action);
    setNotice("");
    try {
      const result = await rpc(action, payload);
      await refresh();
      if (success) setNotice(success);
      return result;
    } catch (e) {
      setNotice(String(e));
      throw e;
    } finally {
      setBusy("");
    }
  };
  const act = (
    action: string,
    payload: Record<string, unknown> = {},
    success?: string,
  ) => void run(action, payload, success).catch(() => {});
  const profile = data?.profiles.find((p) => p.id === selected);
  const create = async () => {
    const p = (await run("create_profile", {
      name: `面试档案 ${(data?.profiles.length ?? 0) + 1}`,
    })) as Profile;
    setSelected(p.id);
    setPage("profiles");
  };
  return (
    <div className="shell">
      <aside className="sidebar">
        <div className="brand">
          <span className="brand-icon">
            <MessageSquareText size={25} />
          </span>
          <div>
            Interview<span>面试字幕助手</span>
          </div>
        </div>
        <div className="sidebar-label">WORKSPACE</div>
        <nav>
          {nav.map(([id, label, Icon]) => (
            <button
              key={id}
              className={page === id ? "nav-item active" : "nav-item"}
              onClick={() => setPage(id)}
            >
              <Icon size={19} />
              {label}
              {page === id && <span className="nav-dot" />}
            </button>
          ))}
        </nav>
        <div className="sidebar-bottom">
          <span className="local-dot" />
          本机保存 · 自带 API Key
          <div>
            Windows Edition <span>v0.1</span>
          </div>
        </div>
      </aside>
      <div className="main">
        <div className="topbar">
          <span>
            <span className="small-dot" />
            个人工作空间
          </span>
          <div className="top-right">
            {preview && (
              <span className="preview-badge">界面预览 · 不调用模型</span>
            )}
            <select
              aria-label="当前编辑档案"
              value={selected}
              onChange={(e) => setSelected(e.target.value)}
            >
              <option value="">选择档案</option>
              {data?.profiles.map((p) => (
                <option key={p.id} value={p.id}>
                  {p.name}
                </option>
              ))}
            </select>
            <button
              className="icon-button"
              title="新建档案"
              onClick={() => void create().catch(() => {})}
            >
              <Plus size={20} />
            </button>
          </div>
        </div>
        <main className="page">
          <header className="page-header">
            <div>
              <div className="eyebrow">
                {page === "home"
                  ? "READY WHEN YOU ARE"
                  : "YOUR INTERVIEW TOOLKIT"}
              </div>
              <h1>{labels[page][0]}</h1>
              <p>{labels[page][1]}</p>
            </div>
            {page === "profiles" && (
              <div className="row">
                <Button
                  secondary
                  onClick={() =>
                    void (async () => {
                      const path = await pickFile(["json"]);
                      if (path) {
                        const p = (await run(
                          "import_profile",
                          { path },
                          "档案已导入",
                        )) as Profile;
                        setSelected(p.id);
                      }
                    })().catch((e) => setNotice(String(e)))
                  }
                >
                  <Upload size={15} />
                  导入档案
                </Button>
                <Button onClick={() => void create().catch(() => {})}>
                  <Plus size={16} />
                  新建档案
                </Button>
              </div>
            )}
          </header>
          {notice && (
            <div className="notice" role="status">
              <span>{notice}</span>
              <button aria-label="关闭提示" onClick={() => setNotice("")}>
                ×
              </button>
            </div>
          )}
          {data?.shortcut_error && (
            <div className="notice" role="alert">
              快捷键未注册：{data.shortcut_error}
              。请在“屏幕与快捷键”修改并保存，或使用首页按钮。
            </div>
          )}
          {busy && (
            <div className="busy">
              <LoaderCircle size={15} className="spin" />
              正在处理…
            </div>
          )}
          {!data ? (
            <Empty
              title="正在连接桌面服务"
              text="请稍候，或检查当前是否在桌面程序中运行。"
            />
          ) : page === "home" ? (
            <Home data={data} go={setPage} act={act} />
          ) : page === "profiles" ? (
            profile ? (
              <ProfileEditor
                key={profile.id}
                profile={profile}
                job={data.jobs.find((j) => j.profile_id === profile.id)}
                run={run}
                notice={setNotice}
              />
            ) : (
              <Empty
                title="从一套面试档案开始"
                text="放入简历、经历素材和回答偏好，整理成自己的表达。"
              >
                <Button onClick={() => void create().catch(() => {})}>
                  <Plus size={16} />
                  创建第一套档案
                </Button>
              </Empty>
            )
          ) : page === "models" ? (
            <Models data={data} run={run} notice={setNotice} />
          ) : page === "screen" ? (
            <Screen
              settings={data.settings}
              run={run}
              ocrStatus={data.ocr_status}
            />
          ) : (
            <Answers
              profile={profile}
              job={
                profile
                  ? data.jobs.find((j) => j.profile_id === profile.id)
                  : undefined
              }
              run={run}
              notice={setNotice}
            />
          )}
        </main>
      </div>
    </div>
  );
}
function Home({
  data,
  go,
  act,
}: {
  data: Snapshot;
  go: (p: Page) => void;
  act: (a: string, p?: Record<string, unknown>, s?: string) => void;
}) {
  const [text, setText] = useState("");
  const active = data.profiles.find(
    (p) => p.id === data.settings.active_profile,
  );
  const checks = [
    {
      ok: !!active?.active,
      title: "启用面试档案",
      text: active?.name ?? "添加资料，确认个人定位和表达要求",
      page: "profiles" as Page,
    },
    {
      ok:
        !!data.settings.answer_connections.length &&
        data.settings.answer_connections.every((id) => data.has_keys[id]),
      title: "连接回答模型",
      text: data.settings.race ? "多模型并发竞速" : "配置自己的 API Key 和模型",
      page: "models" as Page,
    },
    {
      ok: !!data.settings.region,
      title: "设定 OCR 区域",
      text: data.settings.region
        ? `${data.settings.region.width} × ${data.settings.region.height} 像素`
        : "框选问题出现的位置，重启后仍会保留",
      page: "screen" as Page,
    },
    {
      ok: data.ocr_ready,
      title: "本地识别引擎",
      text: data.ocr_status,
      page: "screen" as Page,
    },
  ];
  const ready = checks.filter((c) => c.ok).length;
  return (
    <>
      <section className="hero">
        <div>
          <span className="pill">当前工作空间</span>
          <h2>{active?.name ?? "下一次表达，从充分准备开始"}</h2>
          <p>
            {active?.active
              ? `${active.active.rules.duration_seconds} 秒回答 · ${active.active.rules.words_per_minute} 词/分钟 · ${active.active.rules.optional_ending ? "含两句可选补充" : "完整正文"}`
              : "先准备个人档案和模型，再框选屏幕上的问题。"}
          </p>
          <div className="hero-actions">
            <Button disabled={ready < 4} onClick={() => act("capture")}>
              <ScanLine size={17} />
              识别并回答
            </Button>
            <Button secondary onClick={() => act("toggle")}>
              <Eye size={17} />
              {data.visible ? "隐藏字幕" : "显示字幕"}
            </Button>
          </div>
        </div>
        <div className="readiness">
          <div>
            {ready}
            <span>/ 4</span>
          </div>
          <p>准备就绪</p>
          <div className="progress">
            <i style={{ width: `${(ready / 4) * 100}%` }} />
          </div>
        </div>
      </section>
      <div className="two-column">
        <section className="card">
          <div className="section-heading">
            <h2>使用前准备</h2>
            <span className="subtle">只需设置一次</span>
          </div>
          <div className="checklist">
            {checks.map((c) => (
              <button key={c.title} onClick={() => go(c.page)}>
                <span className={`check-circle ${c.ok ? "done" : ""}`}>
                  {c.ok ? <Check size={15} /> : <span />}
                </span>
                <div>
                  <strong>{c.title}</strong>
                  <p>{c.text}</p>
                </div>
                <ArrowRight size={17} />
              </button>
            ))}
          </div>
        </section>
        <section className="card">
          <div className="section-heading">
            <h2>两个快捷键，专注回答</h2>
            <Keyboard size={19} />
          </div>
          <div className="shortcut-row">
            <div>
              <strong>识别并回答</strong>
              <p>同题复用，新题更新</p>
            </div>
            <kbd>{data.settings.capture_shortcut}</kbd>
          </div>
          <div className="shortcut-row">
            <div>
              <strong>显示 / 隐藏字幕</strong>
              <p>后台处理不受影响</p>
            </div>
            <kbd>{data.settings.toggle_shortcut}</kbd>
          </div>
          <p className="hint-box">
            回答超过 5 秒仍会继续等待。新问题出现后，旧回答不会覆盖当前内容。
          </p>
        </section>
      </div>
      <section className="card">
        <div className="section-heading">
          <h2>试运行与当前内容</h2>
          <span className="subtle">
            {data.caption.cached
              ? "缓存命中"
              : data.caption.elapsed_ms
                ? `${(data.caption.elapsed_ms / 1000).toFixed(2)} 秒`
                : ""}
          </span>
        </div>
        <div className="inline">
          <input
            aria-label="试运行问题"
            placeholder="也可以粘贴一道问题，测试档案和模型…"
            value={text}
            onChange={(e) => setText(e.target.value)}
          />
          <Button
            disabled={!text.trim() || !active?.active}
            onClick={() => act("submit_text", { question: text })}
          >
            <Play size={15} />
            试答
          </Button>
        </div>
        {data.caption.question && (
          <div className="answer-preview">
            <strong>{data.caption.question}</strong>
            <p>{data.caption.answer || data.caption.detail}</p>
            <div className="row">
              <span className="subtle">{data.caption.detail}</span>
              <Button
                secondary
                onClick={() =>
                  act("submit_text", {
                    question: data.caption.question,
                    force: true,
                  })
                }
              >
                重新生成
              </Button>
            </div>
          </div>
        )}
        <div className="row footer-actions">
          <span className="subtle">
            识别失败可手动填写问题，或将上一次选区截图交给云端识别。
          </span>
          <Button secondary onClick={() => act("cloud_retry")}>
            云端重新识别选区
          </Button>
        </div>
      </section>
    </>
  );
}

function JobProgress({ job, run }: { job?: Job; run: Run }) {
  if (!job) return null;
  return (
    <div className="job-status">
      <div className="row">
        <span>
          {job.status === "running" && (
            <LoaderCircle size={15} className="spin" />
          )}{" "}
          {job.detail}
        </span>
        {job.status === "running" && (
          <Button
            secondary
            onClick={() =>
              void run("cancel_job", { id: job.profile_id }).catch(() => {})
            }
          >
            取消后续处理
          </Button>
        )}
      </div>
      <div className="progress">
        <i
          style={{ width: `${job.total ? (job.step / job.total) * 100 : 0}%` }}
        />
      </div>
    </div>
  );
}
function ProfileEditor({
  profile,
  job,
  run,
  notice,
}: {
  profile: Profile;
  job?: Job;
  run: Run;
  notice: (s: string) => void;
}) {
  const [p, setP] = useState(profile),
    [tab, setTab] = useState("materials"),
    [paste, setPaste] = useState(""),
    [sourceName, setSourceName] = useState("个人补充材料");
  useEffect(() => setP(profile), [profile]);
  const working = job?.status === "running";
  const saveProfile = () =>
    run("save_profile", { profile: p }, "档案草稿已保存");
  const prepare = async () => {
    await saveProfile();
    await run("prepare_profile", { id: p.id });
    setTab("review");
  };
  const updateDraft = (part: Partial<Compiled>) =>
    setP({ ...p, draft: { ...(p.draft ?? emptyCompiled), ...part } });
  const importFile = async () => {
    await saveProfile();
    const path = await pickFile([
      "txt",
      "md",
      "docx",
      "pdf",
      "png",
      "jpg",
      "jpeg",
      "webp",
    ]);
    if (path) await run("import_source", { profile_id: p.id, path });
  };
  const exportProfile = async () => {
    await saveProfile();
    const path = await saveFile(`${p.name}.interview.json`, ["json"]);
    if (path)
      await run(
        "export_profile",
        { id: p.id, path },
        "档案已导出，不包含 API Key",
      );
  };
  return (
    <>
      <div className="row page-actions">
        <div className="tabs">
          {[
            ["materials", "个人资料"],
            ["rules", "回答要求"],
            ["review", "整理预览"],
          ].map(([id, label]) => (
            <button
              key={id}
              className={tab === id ? "active" : ""}
              onClick={() => setTab(id)}
            >
              {label}
            </button>
          ))}
        </div>
        <div className="row">
          <Button
            secondary
            disabled={!p.active}
            onClick={() =>
              void (async () => {
                const path = await saveFile(`${p.name}-prompt.md`, ["md"]);
                if (path)
                  await run(
                    "export_prompt",
                    { id: p.id, path },
                    "提示词已导出",
                  );
              })().catch((e) => notice(String(e)))
            }
          >
            导出提示词
          </Button>
          <Button
            secondary
            onClick={() => void exportProfile().catch((e) => notice(String(e)))}
          >
            <Download size={15} />
            导出档案
          </Button>
          <button
            className="icon-button"
            title="删除档案"
            disabled={working}
            onClick={() => {
              if (
                window.confirm(
                  `删除“${p.name}”及其中全部资料和答案？此操作不可撤销。`,
                )
              )
                void run("delete_profile", { id: p.id }, "档案已删除").catch(
                  () => {},
                );
            }}
          >
            <Trash2 size={16} />
          </button>
        </div>
      </div>
      <JobProgress job={job} run={run} />
      <fieldset disabled={working} className="unboxed">
        <section className="card">
          <div className="form-grid">
            <Field label="档案名称">
              <input
                value={p.name}
                onChange={(e) => setP({ ...p, name: e.target.value })}
              />
            </Field>
            <Field
              label="面试目标"
              hint="例如：某学校的硕士面试、软件工程师岗位。"
            >
              <input
                value={p.goal}
                onChange={(e) => setP({ ...p, goal: e.target.value })}
              />
            </Field>
          </div>
        </section>
        {tab === "materials" && (
          <>
            <section className="card">
              <div className="section-heading">
                <div>
                  <h2>资料文件</h2>
                  <p>
                    简历、个人陈述、经历记录、项目介绍。支持扫描 PDF 和图片。
                  </p>
                </div>
                <Button
                  secondary
                  onClick={() =>
                    void importFile().catch((e) => notice(String(e)))
                  }
                >
                  <Upload size={16} />
                  添加文件
                </Button>
              </div>
              {p.sources.length ? (
                <div className="source-list">
                  {p.sources.map((s) => (
                    <div className="source-row" key={s.id}>
                      <FileText size={20} />
                      <div>
                        <strong>{s.name}</strong>
                        <p>
                          {s.extracted
                            ? "已读取，后续整理会复用"
                            : s.blob
                              ? "等待多模态模型读取"
                              : `${s.text.length.toLocaleString()} 字符 · 等待整理`}
                        </p>
                      </div>
                      <button
                        className="icon-button"
                        title="移除资料"
                        onClick={() =>
                          setP({
                            ...p,
                            sources: p.sources.filter((x) => x.id !== s.id),
                            draft: null,
                          })
                        }
                      >
                        <Trash2 size={16} />
                      </button>
                    </div>
                  ))}
                </div>
              ) : (
                <div className="upload-empty">
                  <FileText size={28} />
                  <p>添加文件，或在下面粘贴长文本</p>
                  <small>
                    单个文件最大 18 MB · TXT / MD / DOCX / PDF / 图片
                  </small>
                </div>
              )}
            </section>
            <section className="card">
              <h2>粘贴个人素材</h2>
              <Field label="素材名称">
                <input
                  value={sourceName}
                  onChange={(e) => setSourceName(e.target.value)}
                />
              </Field>
              <textarea
                aria-label="个人素材正文"
                rows={6}
                placeholder="可以粘贴一段项目经历、个人偏好，或你之前准备的长篇材料…"
                value={paste}
                onChange={(e) => setPaste(e.target.value)}
              />
              <div className="row footer-actions">
                <span className="subtle">
                  材料中的信息会先整理为草稿，确认后才用于现场回答。
                </span>
                <Button
                  secondary
                  disabled={!paste.trim()}
                  onClick={() => {
                    setP({
                      ...p,
                      sources: [
                        ...p.sources,
                        {
                          id: uid(),
                          name: sourceName || "补充材料",
                          mime: "text/plain",
                          text: paste,
                          blob: null,
                          digest: "",
                          extracted: "",
                        },
                      ],
                      draft: null,
                    });
                    setPaste("");
                  }}
                >
                  加入资料
                </Button>
              </div>
            </section>
          </>
        )}
        {tab === "rules" && (
          <RulesEditor
            rules={p.rules}
            setRules={(rules) => setP({ ...p, rules })}
          />
        )}
        {tab === "review" && (
          <section className="card">
            <div className="section-heading">
              <div>
                <h2>确认你的面试档案</h2>
                <p>
                  可以直接修改整理结果；不确定的内容留空，不会自动变成个人经历。
                </p>
              </div>
              {p.active && <span className="pill green">已有可用版本</span>}
            </div>
            {!!p.draft?.questions.length && (
              <div className="hint-box">
                <strong>整理时发现这些问题</strong>
                <ol>
                  {p.draft.questions.map((q, i) => (
                    <li key={i}>{q}</li>
                  ))}
                </ol>
              </div>
            )}
            <Field
              label="补充回答或个人背景"
              hint="可以集中回答上面的问题。留空代表跳过；填写后点击“重新整理”纳入档案。"
            >
              <textarea
                rows={4}
                value={p.clarification}
                onChange={(e) => setP({ ...p, clarification: e.target.value })}
              />
            </Field>
            <Field label="个人定位">
              <textarea
                rows={3}
                value={p.draft?.identity ?? ""}
                onChange={(e) => updateDraft({ identity: e.target.value })}
              />
            </Field>
            <div className="form-grid">
              <Field label="确认的个人事实（每行一条）">
                <textarea
                  rows={6}
                  value={p.draft?.facts.join("\n") ?? ""}
                  onChange={(e) =>
                    updateDraft({ facts: e.target.value.split("\n") })
                  }
                />
              </Field>
              <Field label="知识与经验边界（每行一条）">
                <textarea
                  rows={6}
                  value={p.draft?.boundaries.join("\n") ?? ""}
                  onChange={(e) =>
                    updateDraft({ boundaries: e.target.value.split("\n") })
                  }
                />
              </Field>
            </div>
            {!!p.draft?.unknowns.length && (
              <p className="hint-box">
                仍未确认：{p.draft.unknowns.join("；")}
              </p>
            )}
            {p.draft?.cards.map((c, i) => (
              <details className="material-card" key={i}>
                <summary>
                  {c.title || `素材 ${i + 1}`}
                  <span>
                    {c.source_ids
                      .map(
                        (id) =>
                          p.sources.find((s) => s.id === id)?.name ??
                          "来源未知",
                      )
                      .join(" · ")}
                  </span>
                </summary>
                <Field label="主题">
                  <input
                    value={c.title}
                    onChange={(e) =>
                      updateDraft({
                        cards: p.draft!.cards.map((x, j) =>
                          j === i ? { ...x, title: e.target.value } : x,
                        ),
                      })
                    }
                  />
                </Field>
                <Field label="素材内容">
                  <textarea
                    rows={5}
                    value={c.text}
                    onChange={(e) =>
                      updateDraft({
                        cards: p.draft!.cards.map((x, j) =>
                          j === i ? { ...x, text: e.target.value } : x,
                        ),
                      })
                    }
                  />
                </Field>
                <Field label="关键词（逗号分隔）">
                  <input
                    value={c.tags.join(", ")}
                    onChange={(e) =>
                      updateDraft({
                        cards: p.draft!.cards.map((x, j) =>
                          j === i
                            ? {
                                ...x,
                                tags: e.target.value
                                  .split(",")
                                  .map((x) => x.trim()),
                              }
                            : x,
                        ),
                      })
                    }
                  />
                </Field>
              </details>
            ))}
            <Button
              secondary
              onClick={() =>
                updateDraft({
                  cards: [
                    ...(p.draft?.cards ?? []),
                    { title: "新素材", text: "", tags: [], source_ids: [] },
                  ],
                })
              }
            >
              <Plus size={15} />
              添加素材卡
            </Button>
          </section>
        )}
        <div className="sticky-actions">
          <span className="subtle">
            {p.active
              ? "修改保存在草稿中，确认启用后替换当前版本。"
              : "完成整理后，确认启用即可开始使用。"}
          </span>
          <div className="row">
            <Button
              secondary
              onClick={() => void saveProfile().catch(() => {})}
            >
              保存草稿
            </Button>
            <Button secondary onClick={() => void prepare().catch(() => {})}>
              {p.draft ? "重新整理" : "开始整理资料"}
            </Button>
            <Button
              disabled={!p.draft?.identity.trim()}
              onClick={() =>
                void (async () => {
                  await saveProfile();
                  await run(
                    "activate_profile",
                    { id: p.id },
                    "档案已确认并启用",
                  );
                })().catch(() => {})
              }
            >
              <Check size={16} />
              确认启用
            </Button>
          </div>
        </div>
      </fieldset>
    </>
  );
}
function RulesEditor({
  rules: r,
  setRules,
}: {
  rules: Rules;
  setRules: (r: Rules) => void;
}) {
  const [start, setStart] = useState<number | null>(null),
    [elapsed, setElapsed] = useState(0);
  const sample =
    "I enjoy working on problems that connect ideas with real needs. When I begin a new project, I first try to understand what people want to achieve. Then I build a small version and test it carefully. If something does not work, I ask for feedback and change one part at a time. This approach helps me learn from mistakes, and it also makes teamwork easier. I still have many things to learn, but I am willing to ask questions and keep improving.";
  useEffect(() => {
    if (start === null) return;
    const id = setInterval(
      () => setElapsed((performance.now() - start) / 1000),
      100,
    );
    return () => clearInterval(id);
  }, [start]);
  const total = Math.round((r.duration_seconds * r.words_per_minute) / 60),
    extra = r.optional_ending ? Math.floor(total / 5) : 0;
  return (
    <section className="card">
      <h2>回答的长度与表达</h2>
      <div className="form-grid three">
        <Field label="目标朗读时长（秒）">
          <input
            type="number"
            min={15}
            max={600}
            value={r.duration_seconds}
            onChange={(e) =>
              setRules({ ...r, duration_seconds: +e.target.value })
            }
          />
        </Field>
        <Field label="语速预设">
          <select
            value={
              [90, 120, 150].includes(r.words_per_minute)
                ? r.words_per_minute
                : "custom"
            }
            onChange={(e) => {
              if (e.target.value !== "custom")
                setRules({ ...r, words_per_minute: +e.target.value });
            }}
          >
            <option value={90}>慢速 · 90 词/分钟</option>
            <option value={120}>中速 · 120 词/分钟</option>
            <option value={150}>快速 · 150 词/分钟</option>
            <option value="custom">自定义 / 试读校准</option>
          </select>
        </Field>
        <Field label="每分钟词数">
          <input
            type="number"
            min={40}
            max={250}
            value={r.words_per_minute}
            onChange={(e) =>
              setRules({ ...r, words_per_minute: +e.target.value })
            }
          />
        </Field>
      </div>
      <p className="hint-box">
        正文约 {total - extra} 词
        {r.optional_ending ? `，两句补充约 ${extra} 词` : ""}
        。实际朗读时间会因停顿和内容而变化。
      </p>
      <details className="material-card">
        <summary>
          用一段试读校准语速<span>不录音，只计时</span>
        </summary>
        <p className="reading-sample">{sample}</p>
        <div className="row">
          <Button
            secondary
            onClick={() => {
              if (start === null) {
                setStart(performance.now());
                setElapsed(0);
              } else {
                const seconds = (performance.now() - start) / 1000;
                setRules({
                  ...r,
                  words_per_minute: Math.max(
                    40,
                    Math.min(
                      250,
                      Math.round((sample.split(/\s+/).length / seconds) * 60),
                    ),
                  ),
                });
                setStart(null);
              }
            }}
          >
            {start === null ? "开始计时试读" : "读完了，应用语速"}
          </Button>
          <span>{elapsed.toFixed(1)} 秒</span>
        </div>
      </details>
      <Field label="回答语言">
        <input
          value={r.language}
          onChange={(e) => setRules({ ...r, language: e.target.value })}
        />
      </Field>
      <Field label="词汇难度">
        <textarea
          rows={2}
          value={r.difficulty}
          onChange={(e) => setRules({ ...r, difficulty: e.target.value })}
        />
      </Field>
      <Field label="表达风格">
        <textarea
          rows={3}
          value={r.style}
          onChange={(e) => setRules({ ...r, style: e.target.value })}
        />
      </Field>
      <Field
        label="自定义提示词"
        hint="填写身份定位、语气、连接词偏好和不希望出现的表达。"
      >
        <textarea
          rows={6}
          value={r.instructions}
          onChange={(e) => setRules({ ...r, instructions: e.target.value })}
        />
      </Field>
      <label className="check-label">
        <input
          type="checkbox"
          checked={r.optional_ending}
          onChange={(e) =>
            setRules({ ...r, optional_ending: e.target.checked })
          }
        />
        正文后附两句可选补充，用 | 分隔；不读补充也应完整。
      </label>
    </section>
  );
}

function Models({
  data,
  run,
  notice,
}: {
  data: Snapshot;
  run: Run;
  notice: (s: string) => void;
}) {
  const [edit, setEdit] = useState<Connection | null>(null),
    [secret, setSecret] = useState(""),
    [params, setParams] = useState("{}"),
    [s, setS] = useState(data.settings);
  useEffect(() => setS(data.settings), [data.settings]);
  const begin = (c?: Connection) => {
    setEdit(
      c ?? {
        id: uid(),
        name: "新模型连接",
        kind: "openai",
        base_url: "https://api.openai.com/v1",
        model: "",
        vision: true,
        pdf: true,
        parameters: {},
      },
    );
    setSecret("");
    setParams(JSON.stringify(c?.parameters ?? {}, null, 2));
  };
  const defaults = {
    openai: "https://api.openai.com/v1",
    compatible: "https://api.example.com/v1",
    gemini: "https://generativelanguage.googleapis.com",
    anthropic: "https://api.anthropic.com",
  };
  return (
    <>
      <section className="card">
        <div className="section-heading">
          <div>
            <h2>你的模型连接</h2>
            <p>密钥保存在系统凭据存储中，不会写入档案导出文件。</p>
          </div>
          <Button onClick={() => begin()}>
            <Plus size={16} />
            添加连接
          </Button>
        </div>
        {!data.connections.length ? (
          <Empty
            title="连接你自己的模型"
            text="可以分别选择整理模型和即时回答模型，也可以共用一个。"
          />
        ) : (
          <div className="connection-list">
            {data.connections.map((c) => (
              <div className="connection-row" key={c.id}>
                <span className="provider-icon">
                  <PlugZap size={20} />
                </span>
                <div>
                  <strong>{c.name}</strong>
                  <p>
                    {c.model} · {c.kind}
                  </p>
                </div>
                <span className={`pill ${data.has_keys[c.id] ? "green" : ""}`}>
                  {data.has_keys[c.id] ? "已保存密钥" : "未填写密钥"}
                </span>
                <Button secondary onClick={() => begin(c)}>
                  编辑
                </Button>
                <Button
                  secondary
                  onClick={() =>
                    void run("test_connection", { id: c.id })
                      .then((r) => {
                        const t = r as { elapsed_ms: number; text: string };
                        notice(
                          `测试完成：${(t.elapsed_ms / 1000).toFixed(2)} 秒 · ${t.text}`,
                        );
                      })
                      .catch(() => {})
                  }
                >
                  测试
                </Button>
              </div>
            ))}
          </div>
        )}
      </section>
      {edit && (
        <section className="card">
          <div className="section-heading">
            <h2>编辑连接</h2>
            <button
              className="icon-button"
              aria-label="取消编辑"
              onClick={() => setEdit(null)}
            >
              ×
            </button>
          </div>
          <div className="form-grid">
            <Field label="连接名称">
              <input
                value={edit.name}
                onChange={(e) => setEdit({ ...edit, name: e.target.value })}
              />
            </Field>
            <Field label="供应商接口">
              <select
                value={edit.kind}
                onChange={(e) => {
                  const kind = e.target.value as Connection["kind"];
                  setEdit({
                    ...edit,
                    kind,
                    base_url: defaults[kind],
                    pdf: kind !== "compatible",
                  });
                }}
              >
                <option value="openai">OpenAI Responses</option>
                <option value="compatible">OpenAI 兼容接口</option>
                <option value="gemini">Google Gemini 原生</option>
                <option value="anthropic">Anthropic Claude 原生</option>
              </select>
            </Field>
          </div>
          <Field label="API 地址">
            <input
              value={edit.base_url}
              onChange={(e) => setEdit({ ...edit, base_url: e.target.value })}
            />
          </Field>
          <div className="form-grid">
            <Field label="模型 ID" hint="使用供应商控制台中的准确模型名称。">
              <input
                placeholder="填写模型 ID"
                value={edit.model}
                onChange={(e) => setEdit({ ...edit, model: e.target.value })}
              />
            </Field>
            <Field
              label="API Key"
              hint={
                data.has_keys[edit.id]
                  ? "留空保留已保存的密钥；更换地址时需重新填写。"
                  : "只会保存在本机系统凭据存储中。"
              }
            >
              <input
                type="password"
                autoComplete="new-password"
                value={secret}
                onChange={(e) => setSecret(e.target.value)}
                placeholder="输入 API Key"
              />
            </Field>
          </div>
          <div className="row">
            <label className="check-label">
              <input
                type="checkbox"
                checked={edit.vision}
                onChange={(e) => setEdit({ ...edit, vision: e.target.checked })}
              />
              支持图片输入
            </label>
            <label className="check-label">
              <input
                type="checkbox"
                checked={edit.pdf}
                onChange={(e) => setEdit({ ...edit, pdf: e.target.checked })}
              />
              支持原生 PDF 输入
            </label>
          </div>
          <details className="material-card">
            <summary>
              高级模型参数<span>例如 reasoning 或 service_tier</span>
            </summary>
            <p className="subtle">
              按对应供应商接口填写 JSON。这里的参数也会用于“测试连接”。
            </p>
            <textarea
              className="code-input"
              rows={5}
              value={params}
              onChange={(e) => setParams(e.target.value)}
            />
          </details>
          <div className="row">
            <Button
              onClick={() =>
                void (async () => {
                  await run(
                    "save_connection",
                    {
                      connection: { ...edit, parameters: JSON.parse(params) },
                      secret,
                    },
                    "模型连接已保存",
                  );
                  setSecret("");
                  setEdit(null);
                })().catch((e) => notice(String(e)))
              }
            >
              保存连接
            </Button>
            {data.connections.some((c) => c.id === edit.id) && (
              <Button
                secondary
                onClick={() =>
                  void run("delete_connection", { id: edit.id })
                    .then(() => setEdit(null))
                    .catch(() => {})
                }
              >
                <Trash2 size={15} />
                删除连接
              </Button>
            )}
          </div>
        </section>
      )}
      <section className="card">
        <h2>分配模型</h2>
        <Field label="资料整理与准备答案模型">
          <select
            value={s.preparation_connection ?? ""}
            onChange={(e) =>
              setS({ ...s, preparation_connection: e.target.value || null })
            }
          >
            <option value="">选择整理模型</option>
            {data.connections.map((c) => (
              <option key={c.id} value={c.id}>
                {c.name} · {c.model}
              </option>
            ))}
          </select>
        </Field>
        <label className="check-label">
          <input
            type="checkbox"
            checked={s.race}
            onChange={(e) => setS({ ...s, race: e.target.checked })}
          />
          开启多模型并发竞速，采用最先完成的有效回答
        </label>
        <div className="model-choices">
          {data.connections.map((c) => (
            <label key={c.id} className="check-label">
              <input
                type={s.race ? "checkbox" : "radio"}
                name="answer_model"
                checked={s.answer_connections.includes(c.id)}
                onChange={(e) =>
                  setS({
                    ...s,
                    answer_connections: s.race
                      ? e.target.checked
                        ? [...s.answer_connections, c.id]
                        : s.answer_connections.filter((x) => x !== c.id)
                      : [c.id],
                  })
                }
              />
              {c.name}
              <span className="subtle">{c.model}</span>
            </label>
          ))}
        </div>
        <Field
          label="最终请求超时（秒）"
          hint="5 秒之后继续等待；到这个时间才结束失败请求，允许再次触发。"
        >
          <input
            type="number"
            min={10}
            max={300}
            value={s.timeout_seconds}
            onChange={(e) => setS({ ...s, timeout_seconds: +e.target.value })}
          />
        </Field>
        <Button
          onClick={() =>
            void run(
              "save_settings",
              {
                settings: {
                  ...s,
                  answer_connections: s.race
                    ? s.answer_connections
                    : s.answer_connections.slice(0, 1),
                },
              },
              "模型分配已保存",
            ).catch(() => {})
          }
        >
          保存模型分配
        </Button>
      </section>
    </>
  );
}

function Screen({
  settings,
  run,
  ocrStatus,
}: {
  settings: Settings;
  run: Run;
  ocrStatus: string;
}) {
  const [s, setS] = useState(settings);
  useEffect(() => setS(settings), [settings]);
  return (
    <>
      <section className="card">
        <div className="section-heading">
          <div>
            <h2>问题读取区域</h2>
            <p>
              {s.region
                ? `${s.region.monitor.id} · ${s.region.width} × ${s.region.height} 像素`
                : "还没有设定 OCR 区域"}
            </p>
          </div>
          <Button onClick={() => void run("select_region").catch(() => {})}>
            <ScanLine size={16} />
            {s.region ? "重新框选" : "框选区域"}
          </Button>
        </div>
        <div className="screen-diagram">
          <Monitor size={54} />
          <div className="screen-caption">字幕区域 · 可移动</div>
          <div className="screen-region">只读取你框选的区域</div>
        </div>
        <p className="subtle">{ocrStatus}</p>
      </section>
      <section className="card">
        <h2>快捷键</h2>
        <div className="form-grid">
          <Field label="识别并回答">
            <input
              value={s.capture_shortcut}
              onChange={(e) => setS({ ...s, capture_shortcut: e.target.value })}
            />
          </Field>
          <Field label="显示 / 隐藏字幕">
            <input
              value={s.toggle_shortcut}
              onChange={(e) => setS({ ...s, toggle_shortcut: e.target.value })}
            />
          </Field>
        </div>
        <p className="subtle">
          格式示例：Ctrl+Alt+Space。被其他软件占用时会提示，并保留原设置。
        </p>
      </section>
      <section className="card">
        <div className="section-heading">
          <h2>字幕样式与位置</h2>
          <Button
            secondary
            onClick={() => void run("adjust_overlay").catch(() => {})}
          >
            <Settings2 size={16} />
            在屏幕上调整
          </Button>
        </div>
        <div className="form-grid three">
          {(
            [
              ["x", "横坐标"],
              ["y", "纵坐标"],
              ["width", "宽度"],
              ["height", "高度"],
              ["font_size", "字号"],
            ] as const
          ).map(([key, label]) => (
            <Field key={key} label={label}>
              <input
                type="number"
                value={s.overlay[key]}
                onChange={(e) =>
                  setS({
                    ...s,
                    overlay: { ...s.overlay, [key]: +e.target.value },
                  })
                }
              />
            </Field>
          ))}
          <Field label="字体">
            <input
              value={s.overlay.font_family}
              onChange={(e) =>
                setS({
                  ...s,
                  overlay: { ...s.overlay, font_family: e.target.value },
                })
              }
            />
          </Field>
        </div>
        <p className="hint-box">
          白底黑字，默认位于屏幕上方。正常使用鼠标穿透；进入调整模式后可以拖动、缩放和滚动查看，完成后恢复穿透。
        </p>
        <Button
          onClick={() =>
            void run(
              "save_settings",
              { settings: s },
              "屏幕与快捷键设置已保存",
            ).catch(() => {})
          }
        >
          保存设置
        </Button>
      </section>
    </>
  );
}

function Answers({
  profile,
  job,
  run,
  notice,
}: {
  profile?: Profile;
  job?: Job;
  run: Run;
  notice: (s: string) => void;
}) {
  const [items, setItems] = useState<Answer[]>([]),
    [questions, setQuestions] = useState(""),
    [edit, setEdit] = useState<Answer | null>(null),
    [importJson, setImportJson] = useState("");
  const load = useCallback(async () => {
    if (profile) setItems(await rpc<Answer[]>("answers", { id: profile.id }));
  }, [profile]);
  useEffect(() => {
    void load();
  }, [load, job?.status]);
  if (!profile?.active)
    return (
      <Empty
        title="先启用一套面试档案"
        text="准备答案会绑定档案和回答要求，避免修改设置后误用旧答案。"
      />
    );
  const current = items.filter((a) => a.revision === profile.active!.revision),
    old = items.length - current.length;
  return (
    <>
      <JobProgress job={job} run={run} />
      <section className="card">
        <div className="section-heading">
          <div>
            <h2>准备一批问题</h2>
            <p>每行一道题，最多 50 道。生成成功后立即启用。</p>
          </div>
          <Button
            secondary
            onClick={() =>
              void run("suggest_questions", { id: profile.id })
                .then((q) => setQuestions((q as string[]).join("\n")))
                .catch(() => {})
            }
          >
            根据档案建议问题
          </Button>
        </div>
        <textarea
          aria-label="准备的问题"
          rows={5}
          value={questions}
          onChange={(e) => setQuestions(e.target.value)}
          placeholder={
            "Tell me about yourself.\nWhy are you interested in this program?"
          }
        />
        <div className="row footer-actions">
          <span className="subtle">可以删掉不需要的建议题，再开始生成。</span>
          <Button
            disabled={!questions.trim() || job?.status === "running"}
            onClick={() =>
              void run("generate_answers", {
                id: profile.id,
                questions: questions
                  .split("\n")
                  .map((q) => q.trim())
                  .filter(Boolean),
              }).catch(() => {})
            }
          >
            生成这些答案
          </Button>
        </div>
        <details className="material-card">
          <summary>
            导入已经写好的问答<span>JSON 格式</span>
          </summary>
          <p className="subtle">
            格式：[{"{"}"question":"问题","text":"正文 | 两句补充"{"}"}]
          </p>
          <textarea
            aria-label="导入问答JSON"
            rows={4}
            className="code-input"
            value={importJson}
            onChange={(e) => setImportJson(e.target.value)}
          />
          <Button
            secondary
            onClick={() =>
              void (async () => {
                await run(
                  "import_answers",
                  { id: profile.id, answers: JSON.parse(importJson) },
                  "问答已导入并启用",
                );
                setImportJson("");
                await load();
              })().catch((e) => notice(String(e)))
            }
          >
            导入并启用
          </Button>
        </details>
      </section>
      <section className="card">
        <div className="section-heading">
          <div>
            <h2>当前版本的答案</h2>
            <p>
              {current.filter((a) => a.prepared).length} 条准备答案 ·{" "}
              {current.filter((a) => !a.prepared).length} 条现场缓存
              {old > 0 ? ` · ${old} 条历史版本保留在档案中` : ""}
            </p>
          </div>
          <Button
            secondary
            onClick={() =>
              void run("clear_cache", { id: profile.id }, "现场缓存已清理")
                .then(load)
                .catch(() => {})
            }
          >
            清理现场缓存
          </Button>
        </div>
        {!current.length ? (
          <Empty
            title="还没有准备答案"
            text="从几道常见问题开始，生成后可以逐条检查和修改。"
          />
        ) : (
          current.map((a) => (
            <article className="answer-item" key={a.id}>
              <div className="row">
                <span className={`pill ${a.enabled ? "green" : ""}`}>
                  {a.prepared ? "准备答案" : "现场缓存"} ·{" "}
                  {a.enabled ? "启用" : "停用"}
                </span>
                <span className="subtle">{a.origin}</span>
              </div>
              <h3>{a.question}</h3>
              {edit?.id === a.id ? (
                <>
                  <Field label="问题">
                    <input
                      value={edit.question}
                      onChange={(e) =>
                        setEdit({ ...edit, question: e.target.value })
                      }
                    />
                  </Field>
                  <textarea
                    rows={7}
                    value={edit.text}
                    onChange={(e) => setEdit({ ...edit, text: e.target.value })}
                  />
                  <div className="row">
                    <Button
                      onClick={() =>
                        void run("save_answer", { answer: edit }, "答案已更新")
                          .then(async () => {
                            setEdit(null);
                            await load();
                          })
                          .catch(() => {})
                      }
                    >
                      保存修改
                    </Button>
                    <Button secondary onClick={() => setEdit(null)}>
                      取消
                    </Button>
                  </div>
                </>
              ) : (
                <p className="answer-text">{a.text}</p>
              )}
              <div className="row footer-actions">
                <span className="subtle">
                  {a.text.split(/\s+/).filter(Boolean).length} 词 · 正文后 |
                  为可选补充
                </span>
                <div className="row">
                  <Button secondary onClick={() => setEdit(a)}>
                    编辑
                  </Button>
                  <Button
                    secondary
                    onClick={() =>
                      void run("save_answer", {
                        answer: { ...a, enabled: !a.enabled },
                      })
                        .then(load)
                        .catch(() => {})
                    }
                  >
                    {a.enabled ? "停用" : "启用"}
                  </Button>
                  <button
                    className="icon-button"
                    title="删除答案"
                    onClick={() =>
                      void run("delete_answer", { id: a.id })
                        .then(load)
                        .catch(() => {})
                    }
                  >
                    <Trash2 size={16} />
                  </button>
                </div>
              </div>
            </article>
          ))
        )}
      </section>
      {old > 0 && (
        <section className="card">
          <div className="section-heading">
            <div>
              <h2>历史版本</h2>
              <p>保留供查看；不会用于当前档案的命中或参考。</p>
            </div>
          </div>
          {items
            .filter((a) => a.revision !== profile.active!.revision)
            .map((a) => (
              <details className="material-card" key={a.id}>
                <summary>
                  {a.question}
                  <span>{a.revision.slice(0, 8)}</span>
                </summary>
                <p className="answer-text">{a.text}</p>
                <Button
                  secondary
                  disabled={job?.status === "running"}
                  onClick={() =>
                    void run("generate_answers", {
                      id: profile.id,
                      questions: [a.question],
                    }).catch(() => {})
                  }
                >
                  按当前档案重新生成
                </Button>
              </details>
            ))}
        </section>
      )}
    </>
  );
}
