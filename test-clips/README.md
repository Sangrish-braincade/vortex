# Test Clips

This directory holds short video clips used by integration tests.

## Required file

| File | Description | How to obtain |
|------|-------------|---------------|
| `sample-gameplay.mp4` | 30-second FPS gameplay clip (1080p60) | Trimmed from source — see below |

## Generating the sample clip

```bash
ffmpeg -i /path/to/source.mp4 -ss 45 -t 30 -c copy test-clips/sample-gameplay.mp4
```

The source file used during development was `valorant_montage_source.mp4`.
Any 1080p60 gameplay clip works fine for testing.

## Note on Git LFS

Test clips are **not committed to the repo** (they're in `.gitignore`).
If you are contributing and need reproducible integration tests, either:

1. Generate your own `sample-gameplay.mp4` using the command above, or
2. Set up Git LFS and push the file:
   ```bash
   git lfs track "test-clips/*.mp4"
   git add test-clips/sample-gameplay.mp4
   ```
