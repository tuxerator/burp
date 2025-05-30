#import "@local/unikn-thesis:1.0.0": *
#import "@preview/lovelace:0.3.0": *
#import "@preview/algorithmic:1.0.0": *
#import "@preview/cetz:0.3.4" as cetz
#import "@preview/algo:0.3.6": algo, i, d, comment, code
#import "@local/cetz-plot:0.1.1"
#import "cetz-figures.typ"
#import "acronyms.typ": acronyms
#import "glossary.typ": glossary

#show: unikn-thesis.with(
  title: "Beer-Paths",
  authors: ((name: "Jakob Sanowski", student-id: "7654321", course: "TINF22B2", course-of-studies: "Informatik"),),
  city: "Konstanz",
  type-of-thesis: "Bachelor Project",
  // displays the acronyms defined in the acronyms dictionary
  at-university: true,
  // if true the company name on the title page and the confidentiality statement are hidden
  bibliography: bibliography("sources.bib"),
  date: datetime.today(),
  // displays the glossary terms defined in the glossary dictionary
  language: "en",
  // en, de
  supervisor: (university: "Prof. Dr. Sabine Storandt"),
  university: "University",
  university-location: "Konstanz",
  university-short: "Uni KN",
  show-declaration-of-authorship: false,
  math-numbering: none,
  // for more options check the package documentation (https://typst.app/universe/package/clean-dhbw)
)

// Edit this content to your liking

= Introduction

In graph theory and computer science, the beer-path problem presents a unique challenge that extends traditional shortest path queries by introducing the necessity to traverse specific vertices, known as "beer vertices."
This problem is particularly relevant in scenarios where paths must include certain checkpoints or resources, analogous to visiting a "beer store" in a network of roads.
The beer-path oracle is a specialized data structure designed to efficiently answer queries related to beer paths, providing all beer verticies which in-path for any two vertices.

This report delves into the performance of a beer-path oracle, exploring its efficiency, scalability, and practical applications.
We begin by outlining the theoretical foundations of the beer-path problem, highlighting its significance in various computational contexts such as network routing and logistics.
The core of this report focuses on the implementation details of the beer-path oracle, including the algorithms and data structures employed to achieve optimal query times.

// #TODO: write more about the analysis
We present a comprehensive performance analysis, evaluating the oracle's response time and memory usage across different types of graphs.
Through empirical testing, we demonstrate the oracle's ability to handle large-scale (lol) graphs and discuss the trade-offs between preprocessing time and query efficiency.
Furthermore, we compare our beer-path oracle with a double dijkstra approach, underscoring its advantages and potential areas for improvement.

The findings of this report contribute to the ongoing research in graph algorithms and data structures, offering insights into the development of efficient pathfinding techniques under constrained conditions.
By understanding the performance characteristics of the beer-path oracle, we aim to provide a robust framework for future advancements in this field.

// = Related Work

= Preliminaries

// #TODO: reference original Paper

In this section we will establish some preliminary concepts and describe the problem itself.
Most of the definitions are taken from #cite(<Ghosh2023>, form: "prose").

#definition("Shortest Distance")[Given source $s$ and destination $t$ nodes, $d_N (s, t)$ denotes the shortest distance between $s$ and $t$.
  $d_N (s, t)$ is obtained by summing over the edge weights along the shortest path between $s$ and $t$.]

#definition("Detour")[Given source $s$ and destination $t$ nodes, let $pi(s, t)$ denote a simple path that is not necessarily the shortest.
  The detour of such a path is the difference in the network distance along $pi(s, t)$ compared to $d_N (s,t)$.
  Furthermore, it is fairly trivial to see that the detour of any path is greater or equal to zero.]

#definition("Detour Bound")[The detours are bounded by a fraction $epsilon$ such that their total distance does not exceed $epsilon * d_N (s,t)$.
  For example, if $epsilon = 0.1$ a detour can be up $10percent$ longer than the shortest path.]

#definition("In-Path POI")[A POI is said to be _in-path_ if there exists a detour bounded by $epsilon$ which passes through said POI.]

== Problem Definition

We are given a road network $G$, set $P$ of $m$ POIs, and a detour bound $epsilon$.
A driver travels from source $s$ and destination $t$, we want to find the set of pois in $p$ that are “in-path”
under the conditions specified.

// NOTE: Limitations probably should be under 'Algorithms & Implementation'
Now that we have provided our problem statement we can also discuss the limitations of our approach.
First, we are interested in retrieving all the POIs that satisfy the detour constraints.
Our focus here is maximising the throughput where one can answer millions of in-path queries a second using a single machine.
Our solution is not geared towards a driver that wants to visit multiple POIs yet stay within the detour bound.
In our model,
the expectation is that the user is presented with the POI choices and may choose one of the in-path POI to visit.
Such examples include coffee shops, restaurants, gas stations,
vaccination clinics, etc. The driver is unlikely to visit another POI of the same kind. Our
work is in the context of the placement of relevant POIs on a map as an opportunistic
service where speed is of the essence, so, the composition of complex trips that include
visiting multiple POIs is not the focus of this work.

= Algorithms & Implementation

In this section we will look at the algorithms we want to compare in this report.
The first algorithm is a double dijkstra exploring from the start and target towards the POIs.
The second algorihm is a parallel version of the dual Dijkstra @Ghosh2023.
The last algorithm uses a in-path oracle @Ghosh2023 for faster query times.

== Double Dijkstra

// #TODO: Description is enough
The double Dijkstra is a Dijkstra variant for finding detours passing through one $p in P$.
We use two seperate instances of Dijkstra starting from the start $s$ and end $t$ node respectively.
The input for both instances are all POIs from $P$ and $t$ for the instance starting from $s$.
We combine the result of both instances by adding the costs from both instances for every $p in P$ together.
It is important to note for the instance starting from $t$ we traverse the edges backwards.

== Parallel Dual Dijkstra

#cite(<Ghosh2023>, form: "prose") proposed the dual Dijkstra algorithm for finding POIs within a specified detour tolerance limit $epsilon$ which we developed a parallel version of.
In order to parallelise the algorithm we run two Dijkstra at the same time starting from the source $s$ and destination $t$ similar to the double Dijkstra.

@par-dual-dijkstra describes the algorithm of both instances.
Each instance uses its own a priority queue $Q$ over the distance to it's respective start node.
Every node $n$ additionally holds the distance to the start and a label which can be accessed with the functions $d(n)$ and $l(n)$.

At the core of this algorithm is the shared data structure #smallcaps[Visited].
This data structure holds all nodes visited by both Dijkstra instances together with a label indicating which instance found the node and the distance to the start node $s$ or $t$ respectively.
The key of this algorithm is in @merge where we add the two distances together.
If this node $n in P$ we mark it as $bb("POI")$ so it gets added to the result.


#figure(
  kind: "algorithm",
  supplement: [Algorithm],
)[
  #pseudocode-list(booktabs: true, numbered-title: [Parallel dual Dijkstra])[
    *Data:* \
    *Result:*
    + *while* $!italic("Q.empty()") && n := italic("Q.front()") && d(n) <= d_N$ *do*
      + *if* $l(n) == bb("POI")$ *then*
        + $italic("result.add()")$
        + continue
      + *end*
      + *if* $#smallcaps[Visited]\(n, l(n)\)$ *do*
        + continue
      + *end*
      + *if* $n_r$ := #smallcaps[Visited]\(n, l(n).inverse()) *do*
        + $d' := d(n) + d(n_r)$
        + #line-label(<merge>) n.distance($d'$)
        + $d_N := min(d_N, d' * (1 + epsilon))$
        + *if* $n in P$ *do*
          + Q.insert(n.label($bb("POI")$))
        + *end*
      + *end*
      + #smallcaps[Visited]\.insert(n)
      + *for* neighbour $v_i$ of $n$ *do*
        + Q.insert($v_i$.label(l($n$)))
      + *end*
    + *end*
    + *return* result
  ]
] <par-dual-dijkstra>

== Beer-Path Oracle

The beer-path oracle proposed by #cite(<Ghosh2023>, form: "prose") aims to reduce query times using precomputed results.
It uses the _spatial coherence_ @Sankaranarayanan2005 property in road networks which observes similar characteristics for nodes spatially adjacent to each other.
Or more percisely the coherence between shortest paths and distances between nodes and their spatial locations @Sankaranarayanan2005 @Sankaranarayanan2009.
We know for a set of source nodes $A$ and destination nodes $B$ they might share the same shortest paths if $A$ and $B$ are sufficiently far apart and the nodes contained in $A$ and $B$ are close together.
This enables determining if a POI is in-path with respect to this group of nodes opposed to single pairs of nodes.

=== In-Path Property

#cetz-figures.fig_in-path <figure-in-path>

In order to define the _in-path_ property for a set of source nodes $A$ and a set of destination nodes $B$ these sets are restricted to be inside of a bounding box containing all nodes.
Let $a_r$ be a randomly chosen representative source node in $A$ and $b_r$ a representative destination node in $B$.
Let $p$ be the POI we want to determine as in-path with respect to the block-pair $(A, B)$ if all shortest-paths from all sources in $A$ to all destinations in $B$ are in-path to $p$.

We start by defining $r_a^F$ as the forward radius of a given block $A$ denoting the farthest distance from $a_r$ to any node.
Similarly, $r_a^B$ defines the backwards radius denotes the farthest distance of any node to $a_r$.
We also define the forward and backwards radius for any block $B$ as $r_b^F$ and $r_b^B$ respectively (see @figure-in-path).
The following lemmas define bounds for the shortest and longest shortest-paths for all shortest-paths from $A$ to $B$.

#lemma("Shortest Shortest Path")[
  Any shortest path between $A$ and $B$ has a length equal to or greater than $ d_N (a_r, b_r) - (r_a^F + r_b^B). $
]

#proof[
  Let $s$ and $t$ be an arbitrary source and destination with $d_N (s, t) < d_N (a_r, b_r)$.
  Now one can consider the path $a_r -> s -> t -> b_r$. Note that $a_r -> s$ is bounded by $r_a^B$ and $t -> b_r$ is bounded by $r_b^F$.
  Following this $d_N (s,t) >= d_N (a_r,b_r) - (r_a^B + r_b^F)$ has to hold.
  If $d_N (s,t) < d_N (a_r,b_r) - (r_a^B + r_b^F)$ then $d_N (a_r,b_r)$ would not be the shortest distance between $a_r$ and $b_r$ because $d_N (a_r, s) <= r_a^B$ and $d_N (t, b_r) <= r_b^F$ which leads to $d_N (a_r,b_r) < d_N (a_r,b_r) - (r_a^B + r_b^F) + (r_a^B + r_b^F) = d_N (a_r, b_r)$ which is a contradiction.
]

#lemma("Longest Shortest Path")[
  Any shortest path between $A$ and $B$ has a length of at most $ d_N (a_r, b_r) + (r_a^B + r_b^F) $
]

#proof[
  Let $s$ and $t$ be an arbitrary source and destination. Then one can define the following path: $s -> a_r -> b_r -> t$. This path is bound by $d_N (a_r, b_r) + (r_a^B + r_b^F)$.
]

#lemma("In-Path Property")[
  A block-pair $(A,B)$ is in-path if the following condition is satisfied and $d_N (a_r, b_r) - (r_a^F + r_b^B) > 0$:
  $ (r_a^B + d_N (a_r,p) + d_N (p, b_r) + r_b^F) / (d_N (a_r, b_r) - (r_a^F + r_b^B)) -1 <= epsilon $
]

#proof[
  For any given node $s$, $t$ in $A, B$, respectively, $d_N (s,t)$ is at least $d_N (a_r, b_r) - (r_a^F + r_b^B)$ (see @lemma-Shortest-Shortest-Path).
  Considering the path $s -> a_r -> p -> b_r -> t$ it has a length of at most $r_a^B + d_N (a_r, p) + d_N (p, b_r) + r_b^F$.
  If $p$ is _in-path_ to $a_r -> b_r$ then we get the following inequality in order for all possible paths in $A, B$ to be _in-path_:
  $
    r_a^B + d_N (a_r, p) + d_N (p, b_r) + r_b^F <= (d_N (a_r, b_r) - (r_a^F + r_b^B)) dot (1 + epsilon) \
    (r_a^B + d_N (a_r, p) + d_N (p, b_r) + r_b^F) / (d_N (a_r, b_r) - (r_a^F + r_b^B)) - 1 <= epsilon
  $
]

Note that the condition $d_N (a_r, b_r) - (r_a^F + r_b^B) > 0$ is omitted by #cite(<Ghosh2023>, form: "prose") but is necessary because $d_N (a_r, b_r)$ can be 0 in which case $d_N (a_r, b_r) - (r_a^F, r_b^B) < 0$ and thus the condition would suddenly be satisfied if $d_N (a_r, b_r)$ is smaller than some specific value.

#lemma("Not In-Path Property")[
  A block pair $(A,B)$ is not _in-path_ if the following condition is satisfied:
  $ (d_N (a_r,p) + d_N (p, b_r) - (r_a^B + r_b^F)) / (d_N (a_r, b_r) + (r_a^B + r_b^F)) -1 >= epsilon $
]

#proof[
  For any given node $s$, $t$ in $A, B$, respectively, $d_N (s,t)$ is at most $d_N (a_r, b_r) + (r_a^B + r_b^F)$ (see @lemma-Longest-Shortest-Path).
  Considering the path $s -> a_r -> p -> b_r -> t$ it has a length of at least $d_N (a_r, p) + d_N (p, b_r) - (r_a^B + r_b^F)$.
  We get the following inequality in order for all possible paths in $A,B$ to not be _in-path_ to $p$:
  $
    d_N (a_r, p) + d_N (p, b_r) - (r_a^B + r_b^F) >= (d_N (a_r, b_r) + (r_a^B + r_b^F)) dot (1 + epsilon) \
    (d_N (a_r, p) + d_N (p, b_r) - (r_a^B + r_b^F)) / (d_N (a_r, b_r) + (r_a^B + r_b^F)) - 1 >= epsilon
  $
]

#show: style-algorithm
#algorithm-figure(
  "In-Path Oracle for a given POI",
  {
    Assign[$R$][root block of the road network]
    Assign[$#math.italic([result])$][$emptyset$]
    Assign[$Q$][${R,R}$]
    While(
      $#math.italic("!Q.empty()")$,
      {
        Assign[$(A,B)$][$#math.italic("Q.pop_front()")$]
        Assign[$s,t$][random node from $A, B$, respectively]
        Assign[$#math.italic("values")$][Compute $d_n (s,t), d_N (s,p), d_N (p,t), r_a^F, r_a^B, r_b^F, r_b^B$]

        If(
          $#math.italic("values.in-path()")$,
          {
            [$#math.italic("resutl.add((A,B))")$]
          },
        )
        If(
          $#math.italic("values.not-in-path()")$,
          {
            [continue]
          },
        )
        ([Subdivide $A$ and $B$ into 4 children blocks. Discard empty children blocks.],)
        ([Insert all children blocks into $Q$],)
      },
    )
  },
) <algo-in-path-oracle>

// #lemma("In-Path Oracle Size")[
//   The size of the in-path oracle for a single $p$ is $O(1 / epsilon^2 n)$ since it is a Well-Seperated Pair Decomposition (WSPD) of the road network.
// ]
//
// #proof[
//   It can be easily seen that the _in-path_ oracle is a WSPD. Looking at @algo-in-path-oracle we can see that each block pair is either _in-path_ or _not-in-path_ or neither.
//   In the later case we subdivide both blocks and
// ]

=== R\*-Tree

In order to get fast query times we used an _R\*-Tree_ @Beckmann1990 for storing the oracle.
The _R\*-Tree_ is a variant of the _R-Tree_ @Guttman1984 which tries to minimize overlap.

The idea behind _R-Trees_ is to group nearby objects into rectangles and in turn store them in a tree similar to a _B-Tree_ (see @fig-r-tree).
Also like in a _B-Tree_ the data is organized into pages of a fixed size.
This enables search similarly to a _B-Tree_ recursively searching through all nodes which bounding boxes are overlapping with the search area.

#figure(caption: [_R-Tree_ for 2D rectangles with a page size of 3])[
  #image("assets/R-tree.svg", height: 100pt)
] <fig-r-tree>

The performance of an _R-Tree_ greatly depends on the overlap of the bounding boxes in the tree.
Generally less overlap leads to better performance.
For this reason the insertion strategy is crucial for achieving good performance.
_R\*-Trees_ try to minimize the overlap by employing insertion strategies which take this into account.
This improves pruning performance, allowing exclusion of whole pages form search more often.
The key for achieving this is based on the observation that _R-Trees_ are highly susceptible to the order in which their entries are inserted.
For this reason the _R\*-Tree_ performs reinsertion of entries to "find" a better suited place in the tree.

In the case of a node overflowing a portion of its entries are removed and reinserted into tree.
To avoid infinite reinsertion, this may only be performed once per level of the tree.


= Experimental Evaluation

The experiments were performed on the AlgoServer.

== Dataset

The road networks used for evaluation were obtained from OpenStreetMap and sanetized of foot-paths to only include one edge per street. We used two datasets in our evaluation, Konstanz with 2282 nodes and 4377 edges and San Francisco with 95092 nodes and 172256 edges. The weight of each directed edge denotes the travel distance between two nodes. Note that _chains_ (or _ways_) are not simplified.

=== Comparitive Experiments

We used the dual Dijkstra as a baseline for comparison similar to #cite(<Ghosh2023>, form: "prose").
We also compared against a simple parallel version of the dual Dijkstra.
Each data point is sampled at random meaning a source and destination node is chosen randomly.
Each query is run 100 times for all approaches and averaged across all runs.
Furthermore, we increase the number of queries in order to measure the throughput of the algorithms.
The set of POIs is uniformly sampled from the nodes in the road network with a rate. The rate is multiplied with the total number of nodes in order to get the number of sampled nodes.


=== Baseline Approach

The dual Dijkstra serves as a baseline for the _in-path_ oracle.
As a query we used the sampled data points consisting of source and destination pairs.

== In-Path Oracle

To measure the performance we examine the size of the oracle with varying the detour limits and road network size as well as the throughput.

=== Varying Detour Limits

To measure the impact of the detour limit on the oracle size we varied the detour limit from 0.05 to 5.
The test were performed on the Konstanz data set consisting of 2282 nodes and 4377 edges.
As we can see in @fig-oracle-size the oracle size is roughly shaped like a bell which makes sense when looking at @lemma-In-Path-Property and @lemma-Not-In-Path-Property.
When $epsilon$ is very small @lemma-Not-In-Path-Property is more easily satisfied.
Similarly when $epsilon$ is very big @lemma-In-Path-Property is satisfied for bigger blocks.
It is important to note #cite(<Ghosh2023>, form: "prose") report much smaller sizes for a graph of this size.
For a graph with 5000 nodes they report a oracle size of a little bit more than 100,000 compared to the 3,010,095 (see @fig-oracle-size) for a graph with 2248 nodes we found.


#figure(
  cetz.canvas({
    import cetz.draw: *
    import cetz-plot: *

    let data = (
      (0.05, 2306971),
      (0.1, 1809040),
      (0.2, 2135750),
      (0.25, 3010095),
      (0.3, 3120583),
      (0.4, 3228360),
      (0.5, 2899808),
      (0.75, 3735497),
      (1, 4470066),
      (2, 4526431),
      (3, 4071887),
      (4, 3743786),
      (5, 3394786),
    )

    let x-tic-list = data
      .enumerate()
      .map(((i, t)) => {
        (i, t.at(0))
      })

    let data-mapped = data
      .enumerate()
      .map(((i, t)) => {
        (i, t.at(1))
      })
    let x-inset = 0.5

    plot.plot(
      size: (10, 10),
      x-min: -x-inset,
      x-max: data-mapped.len() + x-inset - 1,
      y-min: 1600000,
      y-max: 4800000,
      x-ticks: x-tic-list,
      x-tick-step: none,
      plot-style: (stroke: kn_seeblau, fill: kn_seeblau35),
      mark-style: (stroke: kn_seeblau, fill: kn_seeblau35),
      {
        plot.add(data-mapped, mark: "o")
      },
    )
  }),
) <fig-oracle-size>

== Throughput Experiment

We tested the throughput of _in-path_ queries on both the baseline dual Dijkstra and the _in-path_ oracle.
The experiments were performed on the Konstanz dataset.
POIs were randomly sampled with a sampling rate from the dataset which was varied throughout the experiment.
We computed the _in-path_ oracle for each POI and inserted it into an R\*-Tree.
Each query was performed on the dual Dijkstra, the parallel dual Dijkstra and the _in-path_ oracle.

We observe a throughput of about 100-200? _in-path_ queries/second for the dual Dijkstra on most POI sampling rates running on only one single thread.
The parallel version performed even worse most of the time.
However, as expected the _in-path_ oracle has a much higher throughput than the dual Dijkstra.
Figure ? clearly shows we get more than one million _in-path_ queries per second for all sampling rates.
This confirms the findings of #cite(<Ghosh2023>, form: "prose").

= Conclusions and Future Work

We look at the solution to the _beer-path_ problem proposed by #cite(<Ghosh2023>, form: "prose") and implemented it in Rust.
We could verify the results with regard to the throughput on small instances.
