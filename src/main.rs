mod coordinate_conversion;
mod pitch;

use std::{
    collections::HashMap,
    rc::Rc,
    sync::{Arc, Mutex},
};


use bevy::{prelude::*, sprite::MaterialMesh2dBundle, window::PrimaryWindow};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use pitch::PhaseAccumulator;

use crate::pitch::plot_pitch;
use crate::pitch::Pitch;

#[derive(Resource)]
struct AudioResource(Arc<Mutex<AudioData>>);

struct AudioData(HashMap<PitchId, Pitch>);


#[derive(Resource, Debug, Clone)]
struct PhaseAccumulators(Arc<Mutex<HashMap<PitchId, PhaseAccumulator>>>);

impl PhaseAccumulators {
    fn new() -> Self {
        Self(Arc::new(Mutex::new(HashMap::new())))
    }
}


#[derive(Component, Clone, PartialEq, Eq, Hash, Debug)]
struct PitchId(Arc<str>);

impl PitchId {
    fn new() -> Self {
        Self(cuid2::cuid().into())
    }
}

#[derive(Component)]
struct AddingPitch {}

#[derive(Event)]
enum PitchesChanged {
    Clear,
    Update(PitchId, Pitch),
    Add(PitchId, Pitch),
}



fn lerp(old: f32, new: f32, progress: f32) -> f32 {
    old + ((new - old) * progress)
}

fn unlerp(bottom: f32, top: f32, progress: f32) -> f32 {
    (progress - bottom) / (top - bottom)
}

fn plus_half_steps(pitch: f32, half_steps: f32) -> f32 {
    pitch * (2.0f32).powf(half_steps / 12.0)
}

fn setup(mut commands: Commands) {
    commands.spawn(Camera2dBundle::default());
}

fn keep_pitches_in_sync(
    mut pitches_changed_evs: EventReader<PitchesChanged>,
    phase_accumulators_resource: Res<PhaseAccumulators>,
    audio_resource: Res<AudioResource>,
) {
    let mut data = audio_resource.0.lock().unwrap();
    let mut phase_accumulators = phase_accumulators_resource.0.lock().unwrap();

    for ev in pitches_changed_evs.read() {
        match ev {
            PitchesChanged::Clear => {
                data.0.clear();
            }
            PitchesChanged::Add(id, pitch) => {
                data.0.insert(id.clone(), pitch.clone());
                phase_accumulators.insert(id.clone(), PhaseAccumulator { phase: 0.0 });
            }
            PitchesChanged::Update(id, pitch) => {
                data.0.insert(id.clone(), pitch.clone());
            }
        }
    }
}

fn handle_mouse(
    mut commands: Commands,
    buttons: Res<Input<MouseButton>>,
    keyboard: Res<Input<KeyCode>>,
    q_windows: Query<&Window, With<PrimaryWindow>>,
    pitch_components: Query<(Entity, &Pitch, &Transform), Without<AddingPitch>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    camera_transform: Query<&Transform, (Without<AddingPitch>, With<Camera>)>,
    camera_projection: Query<&OrthographicProjection, With<Camera>>,
    mut pitch_changed_writer: EventWriter<PitchesChanged>,
    mut adding_pitch: Query<(Entity, &mut Pitch, &mut Transform, &PitchId), With<AddingPitch>>,
) {
    if keyboard.just_pressed(KeyCode::Q) {
        if let Ok(c) = adding_pitch.get_single() {
            commands.entity(c.0).despawn();
        }
        pitch_components.for_each(|c| commands.entity(c.0).despawn());
        pitch_changed_writer.send(PitchesChanged::Clear)
    }

    let window = q_windows.single();
    let window_width = window.width();
    let window_height = window.height();

    if let Some(position) = window.cursor_position() {
        let ndc = coordinate_conversion::screen_to_ndc(position, window, 0.0);

        let cam_transform = camera_transform.single();
        let cam_projection = camera_projection.single();

        let world_pos = coordinate_conversion::ndc_to_world(ndc, cam_transform, cam_projection);

        let the_adding_pitch = adding_pitch.get_single_mut();
        let frequency_from_mouse = lerp(130.8262, 523.2511, unlerp(0.0, window_width, position.x));

        if let Ok(mut last_pitch) = the_adding_pitch {
            match last_pitch.1.as_mut() {
                Pitch::Sine { frequency, volume } => {
                    *frequency = frequency_from_mouse;
                    *volume = position.y / window_height;
                }
            }
            last_pitch.2.translation = Vec3::new(world_pos.x, world_pos.y, 0.0);
            pitch_changed_writer.send(PitchesChanged::Update(
                last_pitch.3.clone(),
                last_pitch.1.clone(),
            ));
        }

        if buttons.just_pressed(MouseButton::Left) {
            let pitch = Pitch::Sine {
                frequency: frequency_from_mouse,
                volume: position.y / window_height,
            };
            let pitch_id = PitchId::new();
            commands.spawn((
                pitch.clone(),
                pitch_id.clone(),
                AddingPitch {},
                MaterialMesh2dBundle {
                    mesh: meshes.add(shape::Circle::new(20.).into()).into(),
                    material: materials.add(ColorMaterial::from(Color::RED)),
                    transform: Transform::from_translation(Vec3::new(
                        world_pos.x,
                        world_pos.y,
                        0.0,
                    )),
                    ..default()
                },
            ));
            pitch_changed_writer.send(PitchesChanged::Add(pitch_id.clone(), pitch.clone()))
        }
        if buttons.just_released(MouseButton::Left) {
            if let Ok(p) = adding_pitch.get_single() {
                commands.entity(p.0).remove::<AddingPitch>();
            };
        }
    }
}

fn main() {
    let original_pitches = Arc::new(Mutex::new(AudioData(HashMap::new())));
    let pitches = original_pitches.clone();

    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .expect("Failed to get audio device");

    let mut supported_configs_range = device
        .supported_output_configs()
        .expect("error while querying configs");
    let supported_config = supported_configs_range
        .next()
        .expect("no supported config?!")
        .with_max_sample_rate();

    let sample_rate: f32 = supported_config.sample_rate().0 as f32;

    let phase_accumulators: PhaseAccumulators = PhaseAccumulators::new();
    let phase_accumulators_clone = phase_accumulators.clone();

    let stream = device
        .build_output_stream(
            &supported_config.into(),
            move |raw_audio_data: &mut [f32], _info: &cpal::OutputCallbackInfo| {
                let mut pitches = pitches.lock().unwrap();
                let mut phase_accumulators = phase_accumulators_clone.0.lock().unwrap();
                for sample in raw_audio_data.iter_mut() {
                    *sample = 0.0;
                    for (id, pitch) in pitches.0.iter_mut() {
                        let phase_accumulator = phase_accumulators.get_mut(id).unwrap();
                        *sample += pitch.wave(phase_accumulator, sample_rate);
                        if *sample > 1.0 {
                            println!("Was greater than one: {}", sample);
                        }
                        pitch.advance(phase_accumulator, sample_rate);
                    }
                    *sample = *sample / (pitches.0.iter().len() as f32);
                }
            },
            move |err| {
                println!("{}", err);
            },
            None,
        )
        .expect("Failed to create stream");

    stream.play().expect("Failed to play");

    App::new()
        .add_plugins(DefaultPlugins)
        .add_event::<PitchesChanged>()
        .insert_resource(AudioResource(original_pitches.clone()))
        .insert_resource(phase_accumulators.clone())
        .add_systems(Startup, setup)
        .add_systems(Update, handle_mouse)
        .add_systems(Update, keep_pitches_in_sync)
        .run();
}


