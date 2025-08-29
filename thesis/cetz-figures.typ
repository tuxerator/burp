#import "@local/unikn-thesis:1.0.0": kn_color
#import "@preview/cetz:0.3.4"
#import "@preview/suiji:0.4.0": *
#import "cetz-elements.typ"

#let fig_in-path = cetz-elements.cetz-figure(
  caption: [Whether $p$ is in-path with respect to all sources in $A$ to destinations in $B$.],
  {
    import kn_color: *
    import cetz.draw: *
    import cetz-elements: *
    set-style(
      circle: (radius: 0.08, fill: black, stroke: none),
      content: (padding: .1),
      mark: (scale: .6),
    )
    scope({
      translate((-3, 0))
      rect((0, 0), (rel: (2, 2)), name: "A")
      content((v => cetz.vector.add(v, (-.3, .3)), "A.south-east"))[$A$]
      circle((0.84, 0.66), name: "s", radius: 0.08, fill: kn_seeblau_d, stroke: none)
      content("s", anchor: "north")[$r_a$]

      arrow("s", (1.7, 1.8), name: "r_af", shift: .1)
      content("r_af", anchor: "north-west", padding: 0)[$r_a^F$]
      arrow((0.2, 1.6), "s", name: "r_ab", shift: .1)
      content("r_ab", anchor: "south-west", padding: 0)[$r_a^B$]
    })
    scope({
      translate((3, -0))
      rect((0, 0), (rel: (2, 2)), name: "B")
      content((v => cetz.vector.add(v, (-.3, .3)), "B.south-east"))[$B$]
      circle((1.34, 1.5), name: "t", radius: 0.08, fill: kn_seeblau_d, stroke: none)
      content("t", anchor: "base-west")[$r_b$]

      arrow("t", (0.2, .3), name: "r_bf", shift: 0.2)
      content("r_bf", anchor: "south-east", padding: 0)[$r_b^F$]
      arrow((1.2, .3), "t", name: "r_bb", shift: -0.2)
      content("r_bb", anchor: "mid-west")[$r_b^B$]
    })

    circle((1, .4), name: "p", fill: kn_bordeaux_d)
    content("p", anchor: "north")[$p$]

    arrow("s", "t", name: "a-b", shift: .2, stroke: kn_grau)

    arrow("s", "p", shift: .2, stroke: kn_bordeaux65)
    arrow("p", "t", shift: .2, stroke: kn_bordeaux65)
  },
)

#let fig_r-tree = cetz-elements.cetz-figure({
  import cetz.draw: *
  import cetz-elements: counted-rect

  let m = 3
  let depht = 3
  let rng = gen-rng-f(42)
  let a = ()
  let b = ()

  rect((0, 0), (2, 2))
  set-viewport((0, 0), (2, 2), bounds: (10, 10))
  for i in range(depht) {
    (rng, a) = uniform-f(rng, high: 4., size: 2)
    (rng, b) = uniform-f(rng, low: 1., high: 3., size: 2)


    counted-rect(a, (rel: b))
  }
})


#let fig_one-way-street = cetz-elements.cetz-figure(
  caption: [To get from $p_2$ to $p_1$ one has to take the long way around],
  {
    import kn_color: *
    import cetz.draw: *
    import cetz-elements: *
    set-style(
      circle: (radius: 0.08, fill: black, stroke: none),
      content: (padding: .1),
      mark: (scale: .6),
    )
    scope({
      rect((0, 0), (rel: (2, 2)), name: "block")
      circle((-1.3, 1.3), name: "p0")
      circle((.3, 1.2), name: "p1")
      content("p1", anchor: "north")[$p_1$]
      circle((1.6, 1.5), name: "p2")
      content("p2", anchor: "north")[$p_2$]
      circle((2.6, 2.5), name: "p3")
      circle((2.6, 5.5), name: "p4")
      circle((0.6, 5.5), name: "p5")
      circle((-0.6, 3.5), name: "p6")

      arrow("p0", "p1", shift: 0, stroke: kn_seeblau)
      arrow("p1", "p2", shift: 0)
      arrow("p2", "p3", shift: -.1, stroke: kn_seeblau)
      arrow("p3", "p4", shift: -.1, stroke: kn_seeblau)
      arrow("p4", "p5", shift: -.1, stroke: kn_seeblau)
      arrow("p5", "p6", shift: -.1, stroke: kn_seeblau)
      arrow("p6", "p0", shift: -.1, stroke: kn_seeblau)
    })
  },
)
#let fig_no-spatial-coherence = cetz-elements.cetz-figure(
  caption: [Depending on the _POI_, $d_D$ can vary for the same block pair and thus be a WSP or not.],
  {
    import kn_color: *
    import cetz.draw: *
    import cetz-elements: *
    set-style(
      circle: (radius: 0.08, fill: kn_seeblau, stroke: none),
      content: (padding: .1),
      mark: (scale: .6),
    )

    group({
      rect((0, 0), (rel: (2, 2)), name: "block_a")
      circle((0.6, .8), name: "s")
      content("s", anchor: "north-east")[$s$]

      rect((2, 0), (rel: (2, 2)), name: "block_b")
      circle((3.4, 1.3), name: "t")
      content("t", anchor: "north-west")[$t$]

      circle("block_a", radius: 4, stroke: black, fill: none)

      circle((2, -5), fill: kn_bordeaux_d, name: "poi")
      content("poi", anchor: "north-west")[_POI_]

      arrow("s", "poi", shift: -.2)
      arrow("poi", "t", shift: -.2)

      arrow("s", "t", shift: .1, stroke: kn_grau, name: "d_N")
      get-ctx(ctx => {
        let (ctx, d_N) = cetz.coordinate.resolve(ctx, "d_N")
        content(cetz.vector.add(d_N, (-.3, 0)), anchor: "south")[$d_N$]
      })
    })
    group({
      set-origin((9, 0))
      rect((0, 0), (rel: (2, 2)), name: "block_a")
      circle((0.6, .8), name: "s")
      content("s", anchor: "north-east")[$s$]

      rect((2, 0), (rel: (2, 2)), name: "block_b")
      circle((3.4, 1.3), name: "t")
      content("t", anchor: "north-west")[$t$]

      circle("block_a", radius: 4, stroke: black, fill: none)

      circle((2, -2), fill: kn_bordeaux_d, name: "poi")
      content("poi", anchor: "north-west")[_POI_]

      arrow("s", "poi", shift: -.2)
      arrow("poi", "t", shift: -.2)

      arrow("s", "t", shift: .1, stroke: kn_grau, name: "d_N")
      get-ctx(ctx => {
        let (ctx, d_N) = cetz.coordinate.resolve(ctx, "d_N")
        content(cetz.vector.add(d_N, (-.3, 0)), anchor: "south")[$d_N$]
      })
    })
  },
)

#let fig_wsp(..args) = {
  cetz-elements.cetz-figure(
    ..args,
    {
      import cetz.draw: *

      group(
        name: "A",
        {
          translate((-6, 0))
          circle((0, 0), radius: 2, name: "circle")
          content("circle.north-east", anchor: "south-west", padding: .1)[$A$]

          line("circle.center", "circle.east", name: "r")
          content("r", anchor: "south", padding: .2)[$r$]
        },
      )

      group(
        name: "B",
        {
          translate((6, -0.4))
          circle((0, 0), radius: 2, name: "circle")
          content("circle.north-west", anchor: "south-east", padding: .1)[$B$]

          line("circle.center", "circle.west", name: "r")
          content("r", anchor: "south", padding: .2)[$r$]
        },
      )

      line("A.east", "B.west", name: "d", stroke: kn_color.kn_bordeaux, mark: (symbol: ">", fill: kn_color.kn_bordeaux))

      content("d", anchor: "south", padding: .2)[$>= s r$]
    },
  )
}

#let fig_packing-lemma(..args) = {
  cetz-elements.cetz-figure(
    ..args,
    {
      import cetz.draw: *

      rect((0, 0), (rel: (1, 1)), name: "block")
      content("block.south", anchor: "north", padding: .1)[$2r$]

      circle("block", radius: 3, name: "sphere")

      line("sphere.center", "sphere.west", name: "r")
      content("r", padding: .1, anchor: "south")[$(s+1)r$]

      grid(("sphere.west", "|-", "sphere.north"), ("sphere.east", "|-", "sphere.south"), stroke: (
          paint: kn_color.kn_seeblau,
          thickness: 0.5pt,
          dash: "dashed",
        ), name: "grid")
    },
  )
}
