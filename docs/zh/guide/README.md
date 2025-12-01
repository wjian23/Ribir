# Ribir 文档 v1

欢迎阅读 Ribir v1 文档。Ribir 是一个非侵入式的 Rust GUI 框架，让您能够从单一代码库构建跨平台应用程序。

## 文档结构

本文档分为以下几个部分：

### 1. 核心概念
深入了解 Ribir 的基础原理。
- **[声明式 UI](./core_concepts/declarative_ui.md)**: 介绍 DSL 语法、`fn_widget!` 宏的使用，以及如何静态组合已定义的组件。
- **[内置属性和 FatObj](./core_concepts/built_in_attributes_and_fat_obj.md)**: 讲解如何使用内置属性，以及 `FatObj` 如何为 Widget 提供通用功能。
- **[状态管理](./core_concepts/state_management.md)**: 讲解 `Stateful` 对象，以及如何读写状态并使用 `$read`、`$write`、`pipe!` 和 `$watch` 在视图中创建响应式绑定。
- **[数据共享和事件](./core_concepts/data_sharing_and_events.md)**: 介绍如何使用 `Provider` 在组件树中自上而下地共享数据，以及使用 `自定义事件` 实现自下而上的事件冒泡。
- **[Widget 系统](./core_concepts/widgets_composition.md)**: 讲解 `Render`、`Compose` 和 `ComposeChild` 之间的区别、`SingleChild` 与 `MultiChild` 结构，以及 Widget 声明阶段的工作原理。
- **[布局系统](./core_concepts/layout.md)**: 介绍布局约束传播机制，以及 `clamp` 属性如何影响布局尺寸。

### 2. 高级主题
面向希望扩展 Ribir 功能或深入了解其内部机制的开发者。
- **[自定义 Widget](./advanced/custom_widgets.md)**: 创建您自己的可复用 Widget。
- **[动画](./advanced/animations.md)**: 为 UI 添加动态效果。
- **[主题](./advanced/theming.md)**: 自定义应用的外观和风格，包括类系统、通用颜色提供者、主题提供者和主题类等核心概念。
- **[Widget](./advanced/widgets.md)**: 介绍框架提供的 Widget 组件。

---

## 贡献
Ribir 是开源项目！如果您在文档中发现问题或希望改进文档，请查看我们的[贡献指南](../../CONTRIBUTING.md)。