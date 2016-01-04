# Usage

```bash
cargo run --release  -- "https://www.flickr.com/services/feeds/photos_public.gne?id=127035051@N06&lang=en-us&format=atom"
cargo run --release -- "http://backend.deviantart.com/rss.xml?q=favby%3ADaystar-Art%2F53487288&type=deviation"
cargo run --release  -- "http://backend.deviantart.com/rss.xml?q=favby%3AMeinFragezeichen%2F66073201&type=deviation"
cargo run --release  -- "http://backend.deviantart.com/rss.xml?q=favby%3Aisider%2F52909647&type=deviation"
cargo run --release  -- "http://backend.deviantart.com/rss.xml?q=favby%3ABrainlessGenie%2F61663025&type=deviation"
cargo run --release  -- "http://backend.deviantart.com/rss.xml?q=favby%3Aharvester89%2F63081077&type=deviation"
cargo run --release  -- "http://backend.deviantart.com/rss.xml?q=favby%3Acatacomb-death%2F66830832&type=deviation"
cargo run --release  -- "http://backend.deviantart.com/rss.xml?q=favby%3AFeliFee%2F49024113&type=deviation"
cargo run --release  -- "http://backend.deviantart.com/rss.xml?q=favby%3ALuLupoo%2F66463902&type=deviation"
cargo run --release  -- "http://backend.deviantart.com/rss.xml?q=favby%3Afractal2cry%2F9186189&type=deviation"
```

# Ideas

* Split pictures into tiles for smoother texture loading
* Keyboad control:
  * Pause
  * Prev/Next
  * Adjust settings (durations)
* Render filenames
