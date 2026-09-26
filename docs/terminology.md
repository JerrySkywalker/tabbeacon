# TabBeacon terminology / 术语表

This versioned glossary defines product wording for v0.8.0. Machine-facing values
such as `ResultReady`, `native`, and `off` remain untranslated.

本表规定 v0.8.0 的产品用语。`ResultReady`、`native`、`off` 等机器接口值保持原文。

| ID | English | 推荐中文 | Project meaning and boundary / 项目定义与边界 |
| --- | --- | --- | --- |
| <a id="tb-t01"></a>TB-T01 | Provider | 提供方 | A coding CLI whose structured evidence is normalized by an admitted adapter; naming a provider does not establish support. / 需通过已准入适配器归一化结构化证据的编码 CLI；仅有名称不代表受支持。 |
| <a id="tb-t02"></a>TB-T02 | CLI | 命令行工具 | The original executable and command used by the operator; TabBeacon does not replace its launcher. / 操作者使用的原始程序及命令；TabBeacon 不替代启动器。 |
| <a id="tb-t03"></a>TB-T03 | Session | 会话 | One provider-native conversation identity, distinct from a process, terminal tab, or repository. / 提供方原生对话身份，不等同于进程、终端标签或仓库。 |
| <a id="tb-t04"></a>TB-T04 | Turn | 轮次 | One evidence-backed unit of user request and agent work within a session; late events from an older turn cannot control a newer one. / 同一会话内由证据确认的一轮请求与工作；旧轮次的迟到事件不能控制新轮次。 |
| <a id="tb-t05"></a>TB-T05 | Workspace | 工作区 | Offline-first project identity; Git identity is its stable specialization. / 离线优先的项目身份，Git 身份是稳定的特化形式。 |
| <a id="tb-t06"></a>TB-T06 | Title | 标签标题 | Terminal tab text; the provider retains native control unless TabBeacon has an effective, owned title channel. / 终端标签文字；仅在 TabBeacon 有效且拥有标题通道时才接管。 |
| <a id="tb-t07"></a>TB-T07 | Tab Color | 标签颜色 | Terminal tab color, independent of title and progress. / 终端标签颜色，独立于标题与进度。 |
| <a id="tb-t08"></a>TB-T08 | Activity | 活动指示 | Evidence-backed indication of ongoing agent work, possibly a title marker or progress ring. / 有证据支持的运行指示，可表现为标题标记或进度环。 |
| <a id="tb-t09"></a>TB-T09 | Progress | 进度状态 | Windows Terminal progress channel; it does not measure percent completion of the user's goal. / Windows Terminal 进度通道，不衡量用户目标的完成百分比。 |
| <a id="tb-t10"></a>TB-T10 | Ownership | 所有权 | Proof that a specific configuration entry or output session belongs to TabBeacon before changing or releasing it; configuration ownership and output ownership are separate. / 修改或释放前证明特定配置项或输出会话归 TabBeacon 所有；配置所有权与输出所有权独立。 |
| <a id="tb-t11"></a>TB-T11 | Native | 原生 | `native` leaves the corresponding presentation channel to the CLI or terminal. It does not suppress the provider's own title. / `native` 将相应显示通道交还 CLI 或终端，不禁止提供方自己的标题。 |
| <a id="tb-t12"></a>TB-T12 | Off | 关闭 | `off` disables TabBeacon output on that channel; it does not disable the provider's own title. / `off` 关闭 TabBeacon 在该通道的输出，不关闭提供方自己的标题。 |
| <a id="tb-t13"></a>TB-T13 | Capability | 能力 | Positively established behavior of a provider and host, distinct from a saved preference. / 经正面证明的提供方和宿主行为，不等同于已保存偏好。 |
| <a id="tb-t14"></a>TB-T14 | Evidence | 证据 | Bounded structured observation with known source and authority; visible text or color alone is not runtime evidence. / 来源和权威性已知的有界结构化观测；可见文字或颜色本身不是运行证据。 |
| <a id="tb-t15"></a>TB-T15 | Hook Trust | Hook 信任 | Manual authorization of a Hook definition; installation, configuration, and trust are separate states. / 对 Hook 定义的人工授权；安装、配置、信任是不同状态。 |
| <a id="tb-t16"></a>TB-T16 | Ready | 就绪 | No active work is known and the CLI can receive work; this says nothing about a result awaiting review. / 当前未证明有工作进行且 CLI 可接收任务；不表示有结果待查看。 |
| <a id="tb-t17"></a>TB-T17 | ResultReady | 结果待查看 | An agent result is available for the user; it does not prove the user's whole development goal is finished. / 代理结果可供用户查看；不证明用户的整个开发目标完成。 |
| <a id="tb-t18"></a>TB-T18 | Warning | 警告 | Structured evidence proves an adverse condition, but terminal failure severity is not proved. / 结构化证据证明异常，但未证明终止性失败。 |
| <a id="tb-t19"></a>TB-T19 | Interrupted | 已中断 | Structured evidence proves the active turn was interrupted. / 结构化证据证明当前轮次被中断。 |
| <a id="tb-t20"></a>TB-T20 | Failed | 已失败 | Structured evidence proves the relevant main turn or session failed; a child tool error alone is insufficient. / 结构化证据证明相关主轮次或会话失败；仅子工具错误不足以判定。 |
| <a id="tb-t21"></a>TB-T21 | Inheritance | 继承 | A provider uses an unspecified setting from the existing user-global default. / 提供方未显式指定的设置沿用现有用户全局默认值。 |
| <a id="tb-t22"></a>TB-T22 | Override | 覆盖值 | A partial provider-specific preference; it must not copy or alter another provider's preference. / 针对提供方的局部偏好，不得复制或修改其他提供方的偏好。 |
| <a id="tb-t23"></a>TB-T23 | Saved configuration | 已保存配置 | Preference bytes stored on disk, independent of integration installation or a running session. / 磁盘中的偏好，独立于集成安装及当前会话。 |
| <a id="tb-t24"></a>TB-T24 | Installed integration | 已安装集成 | Owned provider integration definitions exist; this does not imply Hook trust or live application. / 已存在归属明确的提供方集成定义，不代表 Hook 已受信任或当前生效。 |
| <a id="tb-t25"></a>TB-T25 | Effective settings | 生效设置 | Resolved preference after provider and host capability constraints; saving a change may require a CLI restart before a live session uses it. / 经提供方和宿主能力约束后的设置；保存更改后当前会话可能需重启 CLI 才能使用。 |

The [v0.8.0 roadmap](../dev_governance_files/ROADMAP_V08.md) and
[acceptance matrix](../dev_governance_files/V080_ACCEPTANCE_MATRIX.md) define the
scope and required proof for these terms.
