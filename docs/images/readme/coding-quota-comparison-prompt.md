# Coding quota comparison — image generation prompts

Generated with the built-in image_gen tool. Final workspace asset: `coding-quota-comparison-ai.png`.

## Initial generation

```text
Use case: infographic-diagram / ads-marketing.
Create ONE premium Chinese marketing infographic for CodeBeat, a Windows AI coding session scheduling app. Intended for a GitHub README, landscape 16:10, high resolution with crisp highly legible Simplified Chinese typography. A polished editorial information design, not a fake app screenshot. Pale icy blue background #f4faff, white panels, electric blue #0078ff, work-time teal #54baa0, muted slate typography. Subtle soft shadows, generous spacing, restrained line icons (code bracket + clock), refined clean grid. No photos, no characters, no stock watermarks.

Exact headline at top: "让 Coding 额度，跟上你的工作节奏"
Small brand wordmark "CodeBeat" at upper left.
Subtitle: "同一个工作日，不同的窗口安排"
Small contextual chip: "示例工作时间 10:00—20:00"

Central composition: TWO clearly separated horizontally stacked comparison panels, both using EXACTLY THE SAME linear horizontal time mapping from 07:30 at left to 22:30 at right. Labels never overlap or wrap. Include a slim teal reference band for work hours 10:00 to 20:00, aligned vertically with both panels. Subtle shaded vertical backdrop highlights 10:00—20:00. There are no lunch intervals in this example.

Panel 1 heading: "原来的使用方式"
Small explanatory sentence: "上班后首次使用，再等待下一轮恢复"
A calm neutral gray/slate segmented horizontal quota timeline: first window begins 10:00 and ends 15:00, second begins 15:00 and ends 20:00. Leave 07:30—10:00 and 20:00—22:30 unfilled. Thin white internal divider at 15:00 only. Label the start/end boundaries accurately: 10:00, 15:00, 20:00. Inside the two equal-width segments write "窗口 1" and "窗口 2". No text implying quota is exhausted or unavailable throughout a five-hour window; the blocks represent window lifetimes.
Concise right-side or beneath-panel note: "工作时段衔接 2 个窗口"

Panel 2 heading: "使用 CodeBeat"
Small explanatory sentence: "提前轻量开启窗口，后续按计划衔接"
A vivid blue segmented horizontal timeline, three equal-width five-hour segments: 07:30—12:30, 12:30—17:30, 17:30—22:30. This mapping is crucial: each five-hour segment exactly one third of the full 15-hour ruler; 10:00 is one sixth of ruler width and 20:00 five sixths. Thin white dividers only at 12:30 and 17:30, no white boundary lines at endpoints. Boundary labels 07:30, 12:30, 17:30, 22:30 all in ONE row below the band. Text inside "窗口 1", "窗口 2", "窗口 3". Outside-work portions 07:30—10:00 and 20:00—22:30 are a lighter blue, clearly continuing the same windows; keep work-time portions saturated blue. Clearly align 10:00 and 20:00 with the work-time reference above.
Concise note: "工作时段衔接 3 个窗口"

Bottom conclusion in modest large type: "提前安排，让工作时间覆盖更多额度窗口"
Small readable footnote EXACTLY: "仅为排程示意，不增加订阅额度；实际恢复与可用量以官方数据为准。"
Second smaller line: "时间轴展示五小时边界，实际执行会等待恢复后再检查。"

Avoid numeric promises like +50%, doubled quota, extra free tokens, guaranteed speed gains. Do not imply bypassing official limits. No app UI mockup. The result should feel like a credible sophisticated product launch graphic: elegant, information-first, easy to understand at a glance, crisp Chinese labels, balanced visual rhythm.
```

## Timeline refinement

```text
Edit this marketing infographic ONLY to correct the mathematical horizontal scale of the two timelines. Preserve all Chinese copy, branding, typography, colors, icons, panels, spacing, footnotes and overall composition. It is essential to use one uniformly linear 07:30—22:30 scale on BOTH rows, and THREE EQUAL WIDTH blue five-hour blocks in the bottom row.

The full ruler from 07:30 to 22:30 spans exactly 15 hours, from approximately x=65 to x=1521 in the supplied 1586px-wide image. Place boundaries at these precise fractions of ruler width:
07:30 = 0%, 10:00 = 16.6667%, 12:30 = 33.3333%, 15:00 = 50%, 17:30 = 66.6667%, 20:00 = 83.3333%, 22:30 = 100%.
For existing image dimensions this corresponds approximately to x=65,308,550,793,1036,1278,1521. Geometry must follow those fractions even if output resolution changes.

Top gray active window interval is 10:00—20:00, from 16.6667% to 83.3333% of ruler width, divided at 15:00=50%. It consists of TWO equal-width blocks, each one third of the total ruler width. Its empty left and right tails are each one sixth of the ruler width.

Bottom blue window boundaries ONLY at 07:30,12:30,17:30,22:30: THREE EQUAL WIDTH BLOCKS each precisely one third of total ruler width. Each block labeled 窗口 1 / 窗口 2 / 窗口 3 respectively, text centered in its own full five-hour block. Within the first and last blocks, faded out-of-work parts are 07:30—10:00 and 20:00—22:30 respectively, each one sixth of full ruler width. Thus the first blue block is exactly half faded, half saturated; the final blue block is exactly half saturated, half faded.

Both teal work-time arrows and shading MUST go from exactly 10:00=16.6667% to 20:00=83.3333%, vertically aligned between both panels. Move the 10:00 and 20:00 labels, shading boundaries and dotted guide lines accordingly. Keep all tick labels on one row. Do not add/remove words or alter any other design element. No arbitrary spacing: equal durations must have equal widths.
```
