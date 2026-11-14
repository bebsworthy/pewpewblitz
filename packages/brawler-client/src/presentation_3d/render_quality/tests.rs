use super::*;

fn catalog() -> RenderQualityCatalog {
    RenderQualityCatalog::embedded().expect("embedded render quality catalog")
}

const CAPABLE: GpuFacts = GpuFacts {
    gl_backend: false,
    downlevel: DownlevelFlags::all(),
    max_bind_groups: 4,
    max_storage_buffers_per_shader_stage: 8,
};

#[test]
fn high_tier_keeps_the_accepted_desktop_look() {
    let high = catalog().profile(QualityTier::High);
    assert_eq!(
        high,
        QualityProfile {
            msaa: MsaaSamples::Sample4,
            hdr: true,
            bloom: true,
            shadow_maps: true,
            foliage_sway: true,
            floor_decals: true,
            particles: ParticleBackend::Gpu,
        }
    );
    let low = catalog().profile(QualityTier::Low);
    assert_eq!(low.particles, ParticleBackend::Cpu);
    assert!(!low.bloom);
}

#[test]
fn catalog_rejects_unknown_fields_bad_schema_and_bloom_without_hdr() {
    let unknown = RENDER_QUALITY_CATALOG.replacen("hdr: true,", "hdr: true, ssao: true,", 1);
    assert!(RenderQualityCatalog::from_ron(&unknown).is_err());
    let schema = RENDER_QUALITY_CATALOG.replace("schema_version: 2", "schema_version: 3");
    assert!(RenderQualityCatalog::from_ron(&schema).is_err());
    let ldr_bloom = RENDER_QUALITY_CATALOG.replacen(
        "msaa: Sample4,\n        hdr: true,",
        "msaa: Sample4,\n        hdr: false,",
        1,
    );
    assert_ne!(ldr_bloom, RENDER_QUALITY_CATALOG);
    assert!(RenderQualityCatalog::from_ron(&ldr_bloom).is_err());
    let eight_samples = RENDER_QUALITY_CATALOG.replacen("Sample4", "Sample8", 1);
    assert!(RenderQualityCatalog::from_ron(&eight_samples).is_err());
}

#[test]
fn compute_particles_need_compute_storage_and_indirect_support_off_gl() {
    assert!(gpu_particles_supported(CAPABLE));
    for incapable in [
        GpuFacts {
            gl_backend: true,
            ..CAPABLE
        },
        GpuFacts {
            downlevel: DownlevelFlags::all() - DownlevelFlags::COMPUTE_SHADERS,
            ..CAPABLE
        },
        GpuFacts {
            downlevel: DownlevelFlags::all() - DownlevelFlags::VERTEX_STORAGE,
            ..CAPABLE
        },
        GpuFacts {
            max_bind_groups: 3,
            ..CAPABLE
        },
        GpuFacts {
            max_storage_buffers_per_shader_stage: 4,
            ..CAPABLE
        },
    ] {
        assert!(!gpu_particles_supported(incapable), "{incapable:?}");
    }
}

#[test]
fn auto_picks_low_without_compute_medium_on_mobile_and_high_on_desktop() {
    let tier = |gpu_particles, mobile| {
        resolve_tier(
            GraphicsQuality::Auto,
            RenderCapabilities {
                gpu_particles,
                mobile,
            },
        )
        .0
    };
    assert_eq!(tier(false, false), QualityTier::Low);
    assert_eq!(tier(false, true), QualityTier::Low);
    assert_eq!(tier(true, true), QualityTier::Medium);
    assert_eq!(tier(true, false), QualityTier::High);
    let desktop = RenderCapabilities {
        gpu_particles: true,
        mobile: false,
    };
    assert_eq!(
        resolve_tier(GraphicsQuality::Low, desktop),
        (QualityTier::Low, "player override")
    );
}

#[test]
fn headless_detection_reports_no_gpu_particles() {
    let world = World::new();
    assert!(!RenderCapabilities::detect(&world).gpu_particles);
}

fn quality_app(preference: GraphicsQuality, gpu_particles: bool) -> (App, Entity, Entity, Entity) {
    let mut app = App::new();
    app.insert_resource(catalog())
        .insert_resource(RenderCapabilities {
            gpu_particles,
            mobile: false,
        })
        .insert_resource(ClientShellSettings {
            graphics_quality: preference,
            ..Default::default()
        })
        .configure_sets(Update, WorldPresentationSet::PrepareAssets)
        .add_systems(
            Update,
            (
                select_render_quality,
                (apply_camera_quality, apply_light_quality)
                    .run_if(resource_exists::<ActiveRenderQuality>),
            )
                .chain(),
        );
    let arena = app
        .world_mut()
        .spawn((ArenaCamera, Msaa::Sample4, Hdr))
        .id();
    let hud = app
        .world_mut()
        .spawn((WorldHudCamera, Msaa::Sample4, Hdr))
        .id();
    let light = app
        .world_mut()
        .spawn((
            DirectionalLight {
                shadow_maps_enabled: true,
                ..default()
            },
            V3WorldMember,
        ))
        .id();
    app.update();
    (app, arena, hud, light)
}

#[test]
fn low_override_applies_live_and_high_restores_the_desktop_look() {
    let (mut app, arena, hud, light) = quality_app(GraphicsQuality::Low, true);
    let active = *app.world().resource::<ActiveRenderQuality>();
    assert_eq!(active.tier, QualityTier::Low);
    assert_eq!(active.particle_backend(), ParticleBackend::Cpu);
    for camera in [arena, hud] {
        assert_eq!(app.world().get::<Msaa>(camera), Some(&Msaa::Off));
        assert!(app.world().get::<Hdr>(camera).is_none());
    }
    assert!(
        !app.world()
            .get::<DirectionalLight>(light)
            .unwrap()
            .shadow_maps_enabled
    );
    assert!(!foliage_sway_enabled(Some(&active)));
    assert!(!bloom_enabled(Some(&active)));

    app.world_mut()
        .resource_mut::<ClientShellSettings>()
        .graphics_quality = GraphicsQuality::High;
    app.update();
    let active = *app.world().resource::<ActiveRenderQuality>();
    assert_eq!(active.tier, QualityTier::High);
    assert_eq!(active.particle_backend(), ParticleBackend::Gpu);
    for camera in [arena, hud] {
        assert_eq!(app.world().get::<Msaa>(camera), Some(&Msaa::Sample4));
        assert!(app.world().get::<Hdr>(camera).is_some());
    }
    assert!(
        app.world()
            .get::<DirectionalLight>(light)
            .unwrap()
            .shadow_maps_enabled
    );
    assert!(bloom_enabled(Some(&active)));
}

#[test]
fn an_incapable_gpu_never_selects_gpu_particles_even_at_high() {
    let (app, ..) = quality_app(GraphicsQuality::High, false);
    let active = app.world().resource::<ActiveRenderQuality>();
    assert_eq!(active.tier, QualityTier::High);
    assert_eq!(active.particle_backend(), ParticleBackend::Cpu);
}
