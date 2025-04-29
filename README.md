# Shred vs GRPC 性能对比

这个项目用于对比 Solana 网络中 Shred 和 GRPC 两种不同数据获取方式的性能差异。

## 功能特点

- 同时监听 GRPC 和 Shred 数据流
- 实时比较两种方式的数据接收延迟
- 统计以下指标：
  - 首先接收数据的比例
  - 落后时的平均延迟
  - 总体平均延迟

## 环境要求

- Rust 1.70.0 或更高版本
- 有效的 Solana 节点访问权限

## 配置说明

在项目根目录创建 `.env` 文件，配置以下环境变量：

```env
GRPC_URL=your_grpc_endpoint
SHRED_URL=your_shred_endpoint
```

## 运行方法

1. 克隆仓库：
```bash
git clone https://github.com/vnxfsc/shred_vs_grcp.git
cd shred_vs_grcp
```

2. 配置环境变量：
```bash
cp .env.example .env
# 编辑 .env 文件，填入实际的端点地址
```

3. 运行程序：
```bash
cargo run
```

## 输出说明

程序运行时会输出以下信息：
- 测试开始时间
- 测试持续时间（默认30秒）
- 测试端点信息
- 性能对比统计结果

## 性能指标说明

- 首先接收比例：表示该方式首先接收到数据的比例
- 落后时平均延迟：当该方式落后时，平均延迟时间（毫秒）
- 总体平均延迟：所有数据点的平均延迟时间（毫秒）

## 注意事项

- 确保网络连接稳定
- 确保有足够的权限访问 Solana 节点
- 测试时间可以根据需要修改代码中的 `duration.as_secs() >= 30` 参数 