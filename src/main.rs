use std::collections::VecDeque;
use std::f32::consts::PI;
use std::time::Duration;

use bevy::prelude::*;
use bevy::time::Stopwatch;
use bevy::window::WindowResolution;

use bevy_xpbd_2d::prelude::*;

const WINDOW_SIZE: Vec2 = Vec2 {
    x: 1280.,
    y: 720.,
};

const PADDLE_SIZE: Vec2 = Vec2 {
    x: 15.,
    y: 60.,
};

const BALL_RADIUS: f32 = 15.;

const INITIAL_FORCE: f32 = 40000000.;
const PADDLE_SPEED: f32 = 500.;

#[derive(Resource, Default)]
struct Score {
    first_player: u8,
    second_player: u8,
}

#[derive(Component, Debug)]
struct Paddle {
    first_player: bool,
    side: Side,
}

#[derive(Component)]
struct Goal {
    first_player: bool,
    side: Side,
}

#[derive(Component)]
struct Ball {
    kind: BallKind,
}

#[derive(Clone, Debug)]
enum BallKind {
    Point,
    Gold,
    Multi,
    SwitchSide,
}

impl BallKind {
    fn is_bonus(&self) -> bool {
        match self {
            Self::Point|Self::Gold => false,
            _ => true
        }
    }

    fn get_radius(&self) -> f32 {
        if self.is_bonus() {
            BALL_RADIUS / 2.
        } else {
            BALL_RADIUS
        }
    }
}

#[derive(Resource, Default)]
struct PointBallCount(u8);

#[derive(Resource)]
struct BallAssets {
    point_ball: Handle<Image>,
}

#[derive(Resource, Default)]
struct BallSpawner(VecDeque<(BallKind, Side)>);

#[derive(PartialEq, Eq, Clone, Debug)]
enum Side {
    Random,
    Left,
    Right,
}

impl Side {
    fn opposite(&self) -> Self {
        match self {
            Self::Left => Self::Right,
            Self::Right => Self::Left,
            _ => Self::Random
        }
    }
}

#[derive(PhysicsLayer)]
enum Layer {
    Wall,
    Net,
    Paddle,
    Ball,
}

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins.set(WindowPlugin {
                primary_window: Some(Window {
                    resolution: WindowResolution::new(WINDOW_SIZE.x, WINDOW_SIZE.y),
                    ..default()
                }),
                ..default()
            }),
            PhysicsPlugins::default()
        ))
        .insert_resource(Gravity::ZERO)
        .add_systems(Startup, setup)
        .add_systems(Update, (
            (
                check_goals,
                (
                    respawn_point_ball,
                    spawn_bonus_ball,
                ),
                spawn_ball,
            ).chain(),
            move_paddle,
            print_score
        ))
        //.add_systems(Update, debug)
        .run();
}

fn setup(
    mut commands: Commands,
    assets: Res<AssetServer>
) {
    // Assets
    commands.insert_resource(BallAssets {
        point_ball: assets.load("ball_blue_large.png"),
    });

    // Spawner
    commands.init_resource::<BallSpawner>();
    commands.init_resource::<PointBallCount>();

    // Score
    commands.init_resource::<Score>();

    // Camera
    commands.spawn(
        Camera2dBundle {
            ..default()
        }
    );

    // Net (middle line)
    let net_box = Vec2::new(5., WINDOW_SIZE.y);
    commands.spawn((
        SpriteBundle {
            sprite: Sprite {
                custom_size: Some(net_box),
                color: Color::WHITE,
                ..default()
            },
            ..default()
        },
        RigidBody::Static,
        Collider::cuboid(net_box.x, net_box.y),
        CollisionLayers::new([Layer::Net], [Layer::Paddle])
    ));

    // Walls
    // Up wall
    spawn_wall(
        &mut commands,
        0.,
        WINDOW_SIZE.y / 2.,
        Vec2::NEG_Y,
        CollisionLayers::new([Layer::Wall], [Layer::Paddle, Layer::Ball]),
        false,
    );
    // Down wall
    spawn_wall(
        &mut commands,
        0.,
        - WINDOW_SIZE.y / 2.,
        Vec2::Y,
        CollisionLayers::new([Layer::Wall], [Layer::Paddle, Layer::Ball]),
        false,
    );
    // Left wall
    spawn_wall(
        &mut commands,
        - WINDOW_SIZE.x / 2.,
        0.,
        Vec2::X,
        CollisionLayers::new([Layer::Wall], [Layer::Paddle]),
        false,
    );
    // Right wall
    spawn_wall(
        &mut commands,
        WINDOW_SIZE.x / 2.,
        0.,
        Vec2::NEG_X,
        CollisionLayers::new([Layer::Wall], [Layer::Paddle]),
        false,
    );
    // Left goal
    spawn_wall(
        &mut commands,
        - (WINDOW_SIZE.x / 2. + 5.),
        0.,
        Vec2::X,
        CollisionLayers::new([Layer::Wall], [Layer::Ball]),
        true,
    );
    // Right goal
    spawn_wall(
        &mut commands,
        WINDOW_SIZE.x / 2. + 5.,
        0.,
        Vec2::NEG_X,
        CollisionLayers::new([Layer::Wall], [Layer::Ball]),
        true,
    );

    // Paddles
    spawn_paddle(&mut commands, true);
    spawn_paddle(&mut commands, false);
}

fn mirror_transform(
    transform: &mut Transform
) {
    transform.translation.x = - transform.translation.x;
}

fn spawn_paddle(
    commands: &mut Commands,
    first_player: bool,
) {
    let mut transform = Transform::from_xyz(-(WINDOW_SIZE.x / 2.) + 20., 0., 5.);
    let (color, side) = if first_player {
        // Not colorblind friendly, use images that look different in black and white
        (Color::ORANGE, Side::Left)
    } else {
        mirror_transform(&mut transform);
        (Color::PURPLE, Side::Right)
    };
    commands.spawn((
        SpriteBundle {
            sprite: Sprite {
                color,
                custom_size: Some(PADDLE_SIZE),
                ..default()
            },
            transform,
            ..default()
        },
        RigidBody::Kinematic,
        Collider::cuboid(PADDLE_SIZE.x, PADDLE_SIZE.y),
        CollisionLayers::new([Layer::Paddle], [Layer::Ball, Layer::Wall, Layer::Net]),
        Restitution::new(1.),
        Paddle {
            first_player,
            side,
        },
    ));
}

fn spawn_wall (
    commands: &mut Commands,
    x: f32,
    y: f32,
    outward_normal: Vec2,
    collision_layers: CollisionLayers,
    goal: bool,
) {
    let mut wall = commands.spawn((
        Transform::from_xyz(x, y, 0.),
        GlobalTransform::default(),
        RigidBody::Static,
        Collider::halfspace(outward_normal),
        collision_layers,
        Restitution::new(1.),
        Friction::ZERO,
    ));

    if goal {
        let left = x < 0.;
        wall.insert(Goal {
            first_player: left,
            side: if left { Side::Left } else { Side::Right }
        });
    }
}

fn spawn_ball (
    mut commands: Commands,
    ball_assets: Res<BallAssets>,
    mut ball_spawner: ResMut<BallSpawner>,
    spatial_query: SpatialQuery,
    mut timer: Local<Timer>,
    time: Res<Time>,
) {
    timer.tick(time.delta());
    if timer.finished() {
        if let Some((ball_kind, _)) = ball_spawner.0.front() {

            let ball_collider = Collider::ball(ball_kind.get_radius());
            let ball_position = Vec2::ZERO;
            let intersections = spatial_query.shape_intersections(
                &ball_collider,
                ball_position,
                0.,
                SpatialQueryFilter::new().with_masks([Layer::Ball, Layer::Paddle]),
            );

            if intersections.is_empty() {
                let (ball_kind, spawn_direction) = ball_spawner.0.pop_front().unwrap();
                
                timer.set_duration(Duration::from_secs(1));
                timer.reset();

                let direction_angle = rand::random::<f32>() * PI/2. - PI/4.;
                let mut direction = Vec2::from_angle(direction_angle);
                if spawn_direction == Side::Left
                    || (spawn_direction == Side::Random && rand::random::<bool>()) {
                    direction.x = -direction.x;
                }

                commands.spawn((
                    SpriteBundle {
                        texture: ball_assets.point_ball.clone(),
                        sprite: Sprite {
                            color: get_ball_color(&ball_kind),
                            custom_size: Some(Vec2::ONE * (BALL_RADIUS * 2.)),
                            ..default()
                        },
                        transform: Transform::from_translation(ball_position.extend(4.)),
                        ..default()
                    },
                    RigidBody::Dynamic,
                    ball_collider,
                    CollisionLayers::new([Layer::Ball], [Layer::Ball, Layer::Paddle, Layer::Wall]),
                    ExternalForce::new(direction * INITIAL_FORCE).with_persistence(false), // Doesn't seem to work sometimes?
                    Restitution::new(1.),
                    Friction::ZERO,
                    LockedAxes::ROTATION_LOCKED,
                    Ball {
                        kind: ball_kind
                    }
                ));
            }
        }
    }
}

fn get_ball_color (
    ball_kind: &BallKind
) -> Color {
    // Not color-blind friendly, replace with specific sprites instead
    match ball_kind {
        BallKind::Point => Color::WHITE,
        BallKind::Gold => Color::YELLOW,
        BallKind::SwitchSide => Color::GREEN,
        BallKind::Multi => Color::BLUE,
    }
}

fn respawn_point_ball (
    mut point_ball_count: ResMut<PointBallCount>,
    mut ball_spawner: ResMut<BallSpawner>,
) {
    if point_ball_count.0 == 0 {
        ball_spawner.0.push_front((get_point_ball(), Side::Random));
        point_ball_count.0 = 1;
    }
}

fn get_point_ball() -> BallKind {
    if rand::random::<f32>() < 0.02 {
        BallKind::Gold
    } else {
        BallKind::Point
    }
}

fn spawn_bonus_ball(
    mut ball_spawner: ResMut<BallSpawner>,
    mut stopwatch: Local<Stopwatch>,
    time: Res<Time>,
) {
    stopwatch.tick(time.delta());
    if stopwatch.elapsed_secs() > 8. {
        let rand = rand::random::<f32>();
        let kind =
            if rand < 0.2 {
                BallKind::Multi
            } else {
                BallKind::SwitchSide
            };
        ball_spawner.0.push_back((kind, Side::Random));
        stopwatch.reset();
    }
}

fn check_goals(
    mut commands: Commands,
    mut collision_event_reader: EventReader<Collision>,
    mut goals: Query<&mut Goal>,
    balls: Query<&Ball>,
    mut score: ResMut<Score>,
    mut point_ball_count: ResMut<PointBallCount>,
    mut paddles: Query<(&mut Position, &mut Paddle)>,
    mut ball_spawner: ResMut<BallSpawner>,
) {
    for Collision(contact) in collision_event_reader.iter() {
        if let Some((goal, _goal_entity, ball, ball_entity)) =
            if let Ok(goal) = goals.get(contact.entity1) {
                if let Ok(ball) = balls.get(contact.entity2) {
                    Some((goal, contact.entity1, ball, contact.entity2))
                } else {
                    None
                }
            } else if let Ok(goal) = goals.get(contact.entity2) {
                if let Ok(ball) = balls.get(contact.entity1) {
                    Some((goal, contact.entity2, ball, contact.entity1))
                } else {
                    None
                }
            } else {
                None
            }
        {
            match ball.kind {
                BallKind::Point => {
                    if goal.first_player {
                        score.first_player += 1;
                    } else {
                        score.second_player += 1;
                    }
                    point_ball_count.0 -= 1;
                }
                BallKind::Gold => {
                    if goal.first_player {
                        score.first_player += 3;
                    } else {
                        score.second_player += 3;
                    }
                    point_ball_count.0 -= 1;
                }
                BallKind::Multi => {
                    ball_spawner.0.push_front((get_point_ball(), goal.side.clone()));
                    ball_spawner.0.push_front((get_point_ball(), goal.side.clone()));
                }
                BallKind::SwitchSide => {
                    let mut paddles = paddles.iter_mut();
                    let (mut first_transform, mut first_paddle) = paddles.next().unwrap();
                    let (mut second_transform, mut second_paddle) = paddles.next().unwrap();

                    first_paddle.side = first_paddle.side.opposite();
                    second_paddle.side = second_paddle.side.opposite();

                    let temp = first_transform.clone();
                    *first_transform = second_transform.clone();
                    *second_transform = temp;

                    for mut goal in goals.iter_mut() {
                        goal.first_player = !goal.first_player;
                    }
                }
            }
            commands.get_entity(ball_entity).unwrap().despawn();
        }
    }
}

fn move_paddle(
    keys: Res<Input<KeyCode>>,
    mut paddles: Query<(&mut LinearVelocity, &Paddle)>,
) {
    // Probably not the correct way to move a KinematicBody, seems to ignore walls and net.
    // I tried modifying the Transform and Position also to no avail.
    for (mut velocity, paddle) in paddles.iter_mut() {
        if paddle.first_player {
            let mut new_velocity = Vec2::ZERO;
            if keys.pressed(KeyCode::Up) {
                new_velocity.y += PADDLE_SPEED;
            }
            if keys.pressed(KeyCode::Down) {
                new_velocity.y -= PADDLE_SPEED;
            }
            if keys.pressed(KeyCode::Left) {
                new_velocity.x -= PADDLE_SPEED;
            }
            if keys.pressed(KeyCode::Right) {
                new_velocity.x += PADDLE_SPEED;
            }
            *velocity = LinearVelocity(new_velocity);
        }
    }
}

fn print_score (
    score: Res<Score>
) {
    if score.is_changed() {
        println!("Score: {}:{}", score.first_player, score.second_player);
    }
}