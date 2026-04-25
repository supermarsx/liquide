use criterion::{Criterion, black_box, criterion_group, criterion_main};

use liquide_plugin_abi::{ABI_VERSION, ExtensionPoint, PluginManifest};
use liquide_plugin_host::config::PluginHostConfig;
use liquide_plugin_host::dispatcher::Dispatcher;
use liquide_plugin_host::host::PluginHost;
use liquide_plugin_host::plugin::PluginId;
use liquide_plugin_host::resources::ResourcePool;
use liquide_plugin_host::runtime::PluginRuntime;

fn make_manifest(index: u64, points: Vec<ExtensionPoint>) -> PluginManifest {
    PluginManifest {
        id: format!("com.bench.plugin-{index}"),
        name: format!("Bench Plugin {index}"),
        version: "1.0.0".into(),
        abi_version: ABI_VERSION,
        extension_points: points,
        requested_memory_bytes: 1024,
    }
}

fn bench_load_unload_100_plugins(c: &mut Criterion) {
    c.bench_function("load_unload_100_plugins", |b| {
        b.iter(|| {
            let mut config = PluginHostConfig::default();
            config.max_plugins = 128;
            let mut host = PluginHost::new(config);
            let mut ids = Vec::with_capacity(100);
            for i in 0..100u64 {
                let id = host
                    .load_plugin(make_manifest(i, vec![ExtensionPoint::InputFilter]))
                    .unwrap();
                ids.push(id);
            }
            for id in ids {
                let _ = black_box(host.unload_plugin(id));
            }
        })
    });
}

fn bench_dispatch_100_plugins(c: &mut Criterion) {
    c.bench_function("dispatch_100_plugins", |b| {
        let mut config = PluginHostConfig::default();
        config.max_plugins = 128;
        let mut host = PluginHost::new(config);
        for i in 0..100u64 {
            host.load_plugin(make_manifest(i, vec![ExtensionPoint::InputFilter]))
                .unwrap();
        }
        b.iter(|| {
            let results = host.dispatch(ExtensionPoint::InputFilter);
            black_box(results);
        })
    });
}

fn bench_resource_alloc_free_1000(c: &mut Criterion) {
    c.bench_function("resource_alloc_free_1000", |b| {
        b.iter(|| {
            let mut pool = ResourcePool::new(1_000_000);
            let mut handles = Vec::with_capacity(1000);
            for i in 0..1000u64 {
                let h = pool.allocate(64, PluginId(i % 10)).unwrap();
                handles.push(h);
            }
            for h in handles {
                let _ = black_box(pool.free(h));
            }
        })
    });
}

fn bench_dispatcher_register_dispatch(c: &mut Criterion) {
    c.bench_function("dispatcher_register_dispatch_200", |b| {
        b.iter(|| {
            let mut d = Dispatcher::new();
            for i in 0..200u64 {
                d.register(ExtensionPoint::InputFilter, PluginId(i), None)
                    .unwrap();
            }
            let results = d.dispatch(ExtensionPoint::InputFilter);
            black_box(results);
        })
    });
}

fn bench_runtime_suspend_resume_cycle(c: &mut Criterion) {
    c.bench_function("runtime_suspend_resume_50", |b| {
        let mut rt = PluginRuntime::new(PluginHostConfig::default());
        let mut ids = Vec::with_capacity(50);
        for i in 0..50u64 {
            let id = rt.load_plugin(make_manifest(i, vec![])).unwrap();
            ids.push(id);
        }
        b.iter(|| {
            for &id in &ids {
                rt.suspend_plugin(id).unwrap();
            }
            for &id in &ids {
                rt.resume_plugin(id).unwrap();
            }
        })
    });
}

criterion_group!(
    benches,
    bench_load_unload_100_plugins,
    bench_dispatch_100_plugins,
    bench_resource_alloc_free_1000,
    bench_dispatcher_register_dispatch,
    bench_runtime_suspend_resume_cycle,
);
criterion_main!(benches);
