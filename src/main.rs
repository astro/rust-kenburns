#[macro_use]
extern crate glium;
extern crate image;
extern crate time;

use std::fs::File;
use glium::{DisplayBuild, Surface};
use glium::glutin::{VirtualKeyCode, ElementState};
use time::now_utc;


fn get_us() -> u64 {
    let now = now_utc().to_timespec();
    now.sec as u64 * 1000000 + now.nsec as u64 / 1000
}


struct Picture {
    texture: glium::texture::SrgbTexture2d,
    aspect_ratio: f32
}

impl Picture {
    pub fn new<F>(filename: &str, display: &F) -> Self
        where F: glium::backend::Facade {
        
        println!("Loading file {}...", filename);
        let t1 = get_us();
        let file = File::open(filename).unwrap();
        let t2 = get_us();
        let image = image::load(file, image::JPEG).unwrap().to_rgba();
        let t3 = get_us();
        let image_dimensions = image.dimensions();
        let t4 = get_us();
        let image = glium::texture::RawImage2d::from_raw_rgba_reversed(image.into_raw(), image_dimensions);
        let t5 = get_us();
        let texture = glium::texture::SrgbTexture2d::new(display, image).unwrap();
        let t6 = get_us();
        println!("Loaded {} in {} + {} + {} + {} us", filename, t2 - t1, t3 - t2, t5 - t4, t6 - t5);
        Picture {
            texture: texture,
            aspect_ratio: image_dimensions.0 as f32 / image_dimensions.1 as f32
        }
    }
}

#[derive(PartialEq, Eq, Clone, Copy, Debug)]
enum PicturePhase {
    Coming,
    There
}

impl PicturePhase {
    pub fn get_duration(&self) -> u64 {
        match self {
            &PicturePhase::There => 3000000,
            _ => 1500000
        }
    }

    pub fn get_total_duration() -> u64 {
        PicturePhase::Coming.get_duration() +
            PicturePhase::There.get_duration()
    }

    pub fn get_duration_offset(&self) -> u64 {
        let mut offset = 0;
        for phase in &[PicturePhase::Coming, PicturePhase::There] {
            if *phase == *self {
                return offset
            } else {
                offset += phase.get_duration();
            }
        }
        offset
    }
}

struct PictureState {
    start: u64
}

impl PictureState {
    pub fn new() -> Self {
        PictureState {
            start: get_us()
        }
    }
    
    pub fn get_phase(&self) -> PicturePhase {
        let time = get_us() - self.start;
        let mut phase_offset = 0;
        for phase in &[PicturePhase::Coming, PicturePhase::There] {
            if time >= phase_offset && time < phase_offset + phase.get_duration() {
                return *phase;
            }
            phase_offset += phase.get_duration();
        }
        PicturePhase::There
    }
    
    pub fn get_t(&self) -> f32 {
        self.get_overflowing_t().min(1.0)
    }

    pub fn get_overflowing_t(&self) -> f32 {
        let now = get_us();
        (now - self.start) as f32 / PicturePhase::get_total_duration() as f32
    }

    pub fn get_phase_t(&self) -> f32 {
        let time = get_us() - self.start;
        let mut phase_offset = 0;
        for phase in &[PicturePhase::Coming, PicturePhase::There] {
            if time >= phase_offset && time <= phase_offset + phase.get_duration() {
                return (time - phase_offset) as f32 / phase.get_duration() as f32;
            }
            phase_offset += phase.get_duration();
        }
        1.0
    }
    
    pub fn get_alpha(&self) -> f32 {
        match self.get_phase() {
            PicturePhase::Coming => self.get_phase_t(),
            _ => 1.0
        }
    }
}

#[derive(Copy, Clone)]
struct Vertex {
    position: [f32; 3],
    tex_coords: [f32; 2],
}
implement_vertex!(Vertex, position, tex_coords);

    
struct Renderer {
    source: Vec<String>,
    source_index: usize,
    display: glium::backend::glutin_backend::GlutinFacade,
    program: glium::Program,
    current: Option<(Picture, PictureState)>,
    next: Option<(Picture, PictureState)>
}

impl Renderer {
    pub fn new(source: Vec<String>) -> Renderer {
        let display = glium::glutin::WindowBuilder::new()
            .with_depth_buffer(24)
            .with_vsync()
            .build_glium().unwrap();

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
    
            void main() {
                gl_FragColor = texture2D(tex, v_tex_coords);
                gl_FragColor.a = alpha;
            }
        "#;
    
        let program = glium::Program::from_source(&display, vertex_shader_src, fragment_shader_src,
                                                  None).unwrap();
        Renderer {
            source: source,
            source_index: 0,
            display: display,
            program: program,
            current: None,
            next: None
        }
    }

    fn load_next_pic(&mut self) -> Picture {
        let filename = &self.source[self.source_index];
        self.source_index += 1;
        if self.source_index >= self.source.len() {
            self.source_index = 0;
        }
        let pic = Picture::new(filename, &self.display);
        pic
    }

    pub fn update(&mut self) -> bool {
        // events
        for ev in self.display.poll_events() {
            match ev {
                glium::glutin::Event::Closed => return false,
                glium::glutin::Event::KeyboardInput(ElementState::Released, _, Some(key))
                    if key == VirtualKeyCode::Escape || key == VirtualKeyCode::Q =>
                    return false,
                ev => println!("ev: {:?}", ev)
            }
        }

        // elapse/rotate
        println!("current: {:?}\tnext: {:?}",
                 self.current.as_ref().map(|&(ref pic, ref pic_state)| (pic_state.get_phase(), pic_state.get_phase_t(), pic_state.get_t())),
                 self.next.as_ref().map(|&(ref pic, ref pic_state)| (pic_state.get_phase(), pic_state.get_phase_t(), pic_state.get_t()))
                );
        let mut rotate_current = false;
        let mut create_next = false;
        match (&self.current, &self.next) {
            (_, &Some((_, ref next_state)))
                if next_state.get_phase() == PicturePhase::There =>
                    rotate_current = true,
            (&None, &None) =>
                create_next = true,
            (&Some((_, ref current_state)), &None)
                if current_state.get_phase() == PicturePhase::There &&
                    current_state.get_phase_t() >= 1.0 =>
                    create_next = true,
            (_, _) => ()
        }
        if rotate_current {
            self.current = self.next.take();
        } else if create_next {
            let pic = self.load_next_pic();
            let pic_state = PictureState::new();
            self.next = Some((pic, pic_state));
        }

        // continue main loop
        true
    }
    
    pub fn render(&self) {
        let mut target = self.display.draw();
        target.clear_color_and_depth((0.0, 0.0, 0.0, 1.0), 1.0);
        // let (width, height) = target.get_dimensions();
        // let aspect_ratio = width as f32 / height as f32;

        
        match self.current {
            None => (),
            Some((ref current_pic, ref current_state)) =>
                self.render_picture(&mut target, current_pic, current_state)
        }
        match self.next {
            None => (),
            Some((ref next_pic, ref next_state)) =>
                self.render_picture(&mut target, next_pic, next_state)
        }

        target.finish().unwrap();
    }

    fn render_picture(&self, target: &mut glium::Frame, pic: &Picture, state: &PictureState) {
        let shape = glium::vertex::VertexBuffer::new(&self.display, &[
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
        let zoom = 1.0 + 0.2 * state.get_overflowing_t();
        matrix[0][0] *= zoom;
        matrix[1][1] *= zoom;
        let params = glium::DrawParameters {
            depth: glium::Depth {
                test: glium::draw_parameters::DepthTest::Overwrite,
                write: false,
                .. Default::default()
            },
            blend: glium::Blend::alpha_blending(),
            .. Default::default()
        };
        target.draw(
            &shape,
            glium::index::NoIndices(glium::index::PrimitiveType::TriangleStrip),
            &self.program,
            &uniform! { matrix: matrix, tex: &pic.texture, alpha: state.get_alpha() as f32 },
            &params
        ).unwrap();
    }
}

fn main() {
    let mut renderer = Renderer::new(
        std::env::args()
        .skip(1)
        .collect()
    );

    let mut tick = 0;
    while renderer.update() {
        renderer.render();
        tick += 1;
    }
}
