# Found
BRL-094c device check 2026-11-15, Pixel 8a, HEAD 6a6d6599, Practice Hot Zone 3v3.

# Observation
- `adb shell input tap 212 90` on the drawn menu button did not open the menu (2 of 2 tries). adb's tap sends down and up within about 1 ms, so both land in the same frame.
- `input swipe 212 90 212 90 150` also failed once.
- `input motionevent DOWN 210 90`, then 200 ms, then `UP`, opened it every time.
- Android Back opened and closed the menu every time.
This matches BRL-rs10's finding for sliders: a touch that begins and ends inside one frame is missed. hcyk noted that Bevy `Touches` keeps `just_pressed` for such touches. At the ~30 fps High tier, or when frames drop, a real quick thumb tap can fall inside one frame.

# Scope
- Treat a touch whose press and release fall in the same frame on the menu rect as a tap (`touch/model.rs` `TouchControlKind::Menu` handling). Add a unit test with a same-frame start/end, like the rs10 slider test.
- Check the other HUD touch buttons for the same pattern. Attack and ultimate taps with 60-80 ms holds worked here.

Priority: low. Real fingers usually span frames at 60 fps.

Evidence: /private/tmp/claude-501/-Users-boyd-wip-brawler/716520d5-661e-45a6-996e-1f74fe351a54/scratchpad/dc/s-m01.jpg, s-m05.jpg, m07-touch-hold-open.jpg, s-q.jpg.
