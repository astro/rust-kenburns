use glium::{Display, Surface};
use glium::glutin::{WindowBuilder, ContextBuilder, EventsLoop, Event, WindowEvent, VirtualKeyCode, ElementState};
use glium::texture::{SrgbTexture2d, RawImage2d};
use glium::{Program, DrawParameters, Depth, Blend, Frame};
use glium::draw_parameters::DepthTest;
use glium::vertex::VertexBuffer;
use glium::index::{NoIndices, PrimitiveType};
use std::ops::Not;
use std::sync::mpsc::Receiver;

use util::*;

const SHOW_DURATION: u64 = 3_000_000;
const TRANSITION_DURATION: u64 = 300_000;

struct Picture {
    texture: SrgbTexture2d
}

impl Picture {
    pub fn new(texture: SrgbTexture2d) -> Self {
        Picture {
            texture: texture
        }
    }

    pub fn get_aspect_ratio(&self) -> f32 {
        self.texture.get_width() as f32 /
            self.texture.get_height().unwrap() as f32
    }
}

#[derive(PartialEq, Eq, Clone, Copy, Debug)]
enum ZoomDirection {
    In,
    Out
}

impl Not for ZoomDirection {
    type Output = ZoomDirection;

    fn not(self) -> ZoomDirection {
        match self {
            ZoomDirection::In => ZoomDirection::Out,
            ZoomDirection::Out => ZoomDirection::In
        }
    }
}

struct PictureState {
    start: u64,
    zoom_direction: ZoomDirection
}

impl PictureState {
    pub fn new(zoom_direction: ZoomDirection) -> Self {
        PictureState {
            start: get_us(),
            zoom_direction: zoom_direction
        }
    }

    pub fn has_transitioned(&self) -> bool {
        (get_us() - self.start) > TRANSITION_DURATION
    }

    pub fn get_overflowing_t(&self) -> f32 {
        let now = get_us();
        (now - self.start) as f32 / SHOW_DURATION as f32
    }

    pub fn get_zoom(&self) -> f32 {
        let time_zoom = match self.zoom_direction {
            ZoomDirection::In =>
                /* Linear zooming in */
                self.get_overflowing_t(),
            ZoomDirection::Out =>
                /* Slowing zoom out
                * that stops before showing black borders
                */
                (1.0 - self.get_overflowing_t())
                .max(0.0)
                .powf(2.0)
        };
        1.0 + 0.1 * time_zoom
    }

    pub fn get_alpha(&self) -> f32 {
        let age = (get_us() - self.start) as f32;
        (age / TRANSITION_DURATION as f32).min(1.0)
    }
}

#[derive(Copy, Clone)]
struct Vertex {
    position: [f32; 3],
    tex_coords: [f32; 2],
}
implement_vertex!(Vertex, position, tex_coords);


pub struct Renderer<'a> {
    source_rx: Receiver<RawImage2d<'a, u8>>,
    display: Display,
    events_loop: EventsLoop,
    program: Program,
    current: Option<(Picture, PictureState)>,
    next: Option<(Picture, PictureState)>
}

impl<'a> Renderer<'a> {
    pub fn new(source_rx: Receiver<RawImage2d<'a, u8>>) -> Renderer<'a> {
        let window = WindowBuilder::new()
            .with_title("Rust<KenBurns>");


        let context = ContextBuilder::new()
            .with_depth_buffer(24)
            .with_vsync(true);
        let events_loop = EventsLoop::new();
        let display = Display::new(window, context, &events_loop).unwrap();
        
        let vertex_shader_src = r#"
            #version 140

            in vec3 position;
            in vec2 tex_coords;

            out vec2 v_tex_coords;

            uniform mat4 matrix;

            void main() {
                v_tex_coords = tex_coords;
                gl_Position = matrix * vec4(position, 1.0);
            }
        "#;

        let fragment_shader_src = r#"
            #version 140

            in vec2 v_tex_coords;
            uniform float alpha;

            uniform sampler2D tex;

            out vec4 frag_color;

            void main() {
                frag_color = texture(tex, v_tex_coords);
                frag_color.a = alpha;
            }
        "#;

        let program = Program::from_source(&display, vertex_shader_src, fragment_shader_src,
                                           None).unwrap();
        Renderer {
            source_rx,
            display,
            events_loop,
            program,
            current: None,
            next: None
        }
    }

    fn load_next_pic(&mut self) -> Option<Picture> {
        let t1 = get_us();
        let image = match self.source_rx.try_recv() {
            Err(_) => return None,
            Ok(image) => image
        };
        let t2 = get_us();
        let texture = SrgbTexture2d::new(&self.display, image).unwrap();
        let t3 = get_us();
        let pic = Picture::new(texture);
        let t4 = get_us();
        println!("Converted pic in {} + {} + {} us", t2 - t1, t3 - t2, t4 - t3);
        Some(pic)
    }

    pub fn update(&mut self) -> bool {
        let mut running = true;
        // events
        self.events_loop.poll_events(|ev| {
            match ev {
                Event::WindowEvent { event, window_id: _ } =>
                    match event {
                        WindowEvent::KeyboardInput { input, device_id: _ }
                        if input.state == ElementState::Released
                            && input.virtual_keycode == Some(VirtualKeyCode::Escape) =>
                            running = false,
                        WindowEvent::Closed =>
                            running = false,
                        _ => (),
                    },
                _ => (),
            }
        });

        // elapse/rotate
        let mut rotate_current = false;
        let mut create_next = false;
        let now = get_us();
        match (&self.current, &self.next) {
            (_, &Some((_, ref next_state)))
                if next_state.has_transitioned() =>
                    rotate_current = true,
            (&None, &None) =>
                create_next = true,
            (&Some((_, ref current_state)), &None)
                if now - current_state.start >= SHOW_DURATION - TRANSITION_DURATION =>
                    create_next = true,
            (_, _) => ()
        }
        if rotate_current {
            self.current = self.next.take();
        } else if create_next {
            self.load_next_pic().map(|pic|{
                let current_direction = self.current
                    .as_ref()
                    .map(|&(_, ref current_state)| current_state.zoom_direction);
                let pic_state = PictureState::new(!current_direction.unwrap_or(ZoomDirection::Out));
                self.next = Some((pic, pic_state));
            });
        }

        running
    }

    pub fn render(&self) {
        let mut target = self.display.draw();
        let (target_width, target_height) = target.get_dimensions();
        let target_aspect_ratio = target_width as f32 / target_height as f32;

        target.clear_color_and_depth((0.0, 0.0, 0.0, 1.0), 1.0);

        match self.current {
            None => (),
            Some((ref current_pic, ref current_state)) =>
                self.render_picture(&mut target, current_pic, current_state, target_aspect_ratio)
        }
        match self.next {
            None => (),
            Some((ref next_pic, ref next_state)) =>
                self.render_picture(&mut target, next_pic, next_state, target_aspect_ratio)
        }

        target.finish().unwrap();
    }

    fn render_picture(&self, target: &mut Frame, pic: &Picture, state: &PictureState, target_aspect_ratio: f32) {
        let shape = VertexBuffer::new(&self.display, &[
            Vertex { position: [-1.0,  1.0, 0.0], tex_coords: [0.0, 1.0] },
            Vertex { position: [ 1.0,  1.0, 0.0], tex_coords: [1.0, 1.0] },
            Vertex { position: [-1.0, -1.0, 0.0], tex_coords: [0.0, 0.0] },
            Vertex { position: [ 1.0, -1.0, 0.0], tex_coords: [1.0, 0.0] },
        ]).unwrap();
        let mut matrix = [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0f32]
        ];
        /* Ratio correction */
        let texture_aspect_ratio = pic.get_aspect_ratio();
        if target_aspect_ratio > texture_aspect_ratio {
            /* Too wide, stretch y: */
            matrix[1][1] *= target_aspect_ratio / texture_aspect_ratio;
        } else {
            /* Too tall, stretch x: */
            matrix[0][0] *= texture_aspect_ratio / target_aspect_ratio;
        };
        /* Zoom */
        let zoom = state.get_zoom();
        matrix[0][0] *= zoom;
        matrix[1][1] *= zoom;
        let params = DrawParameters {
            depth: Depth {
                test: DepthTest::Overwrite,
                write: false,
                .. Default::default()
            },
            blend: Blend::alpha_blending(),
            .. Default::default()
        };
        target.draw(
            &shape,
            NoIndices(PrimitiveType::TriangleStrip),
            &self.program,
            &uniform! { matrix: matrix, tex: &pic.texture, alpha: state.get_alpha() as f32 },
            &params
        ).unwrap();
    }
}
