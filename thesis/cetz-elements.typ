#import "@preview/cetz:0.3.4" as cetz: *
#let arrow(..points-style, shift: 0.5, name: none) = {
  import draw: *
  let style = points-style.named()
  let points = points-style.pos()
  assert(
    points.len() == 2,
    message: "arrow expects two points, got " + repr(points),
  )

  group(
    name: name,
    ctx => {
      let default_style = (
        fill: none,
        stroke: black,
        mark: (
          end: "stealth",
          fill: if style.keys().contains("stroke") {
            style.stroke
          } else {
            black
          },
        ),
      )

      let style = cetz.styles.resolve(ctx.style, merge: style, base: default_style, root: "arrow")
      let (s, t) = points
      let (ctx, s_v, t_v) = cetz.coordinate.resolve(ctx, s, t)
      let normal = cetz.util.line-normal(s_v, t_v)
      let mid_point = cetz.util.line-pt(s_v, t_v, .5)
      let shift_point = cetz.vector.add(mid_point, cetz.vector.scale(normal, shift))

      if type(s) == str {
        intersections(
          "i",
          s,
          hide(catmull(s_v, shift_point, t_v, tension: .5)),
        )
        anchor("arrow_s_i", "i.0")
      } else {
        anchor("arrow_s_i", s_v)
      }
      if type(t) == str {
        intersections(
          "i",
          t,
          hide(catmull(s_v, shift_point, t_v, tension: .5)),
        )
        anchor("arrow_t_i", "i.0")
      } else {
        anchor("arrow_t_i", t_v)
      }

      catmull("arrow_s_i", shift_point, "arrow_t_i", name: name, tension: .5, ..style)

      anchor("default", shift_point)
    },
  )
}

#let counted-rect(a, b, name: none, ..style) = {
  import cetz.draw: *
  // No extra positional arguments from the style sink
  assert.eq(
    style.pos(),
    (),
    message: "Unexpected positional arguments: " + repr(style.pos()),
  )
  let style = style.named()
  let default_style = (stroke: black)

  group(
    name: name,
    ctx => {
      let c = counter("counted-rect")
      let style = cetz.styles.resolve(ctx.style, merge: style, base: default_style, root: "counted-rect")
      set-style(..style)
      rect(a, b, name: "rect")
      content("rect.north-west", anchor: "north-west", padding: .2)[#c.step() R#context c.display()]
    },
  )
}

#let cetz-figure(..args, body) = {
  figure(..args)[
    #context {
      let (width, height) = measure(cetz.canvas(body))
      layout(size => {
        if width > size.width {
          cetz.canvas(length: (size.width / width) * 1cm, body)
        } else {
          cetz.canvas(body)
        }
      })
    }
  ]
}
