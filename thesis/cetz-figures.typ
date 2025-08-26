#import "@local/unikn-thesis:1.0.0": kn_color
#import "@preview/cetz:0.3.4"
#import "@preview/suiji:0.4.0": *
#import "cetz-elements.typ"

#let fig_in-path = figure(caption: [Whether $p$ is in-path with respect to all sources in $A$ to destinations in $B$.])[
  #cetz.canvas({
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
  })
];

#let fig_r-tree = figure()[
  #cetz.canvas({
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
]

#let fig_one-way_street = figure(caption: [To get from $p_2$ to $p_1$ one has to take the long way around])[
  #cetz.canvas({
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
  })
]
