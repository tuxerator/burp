#import "@local/unikn-thesis:1.0.0": *
#import "@preview/lovelace:0.3.0": *
#import "@preview/algorithmic:1.0.4": *
#import "@preview/cetz:0.3.4" as cetz
#import "@preview/algo:0.3.6": algo, i, d, comment, code
#import "@local/cetz-plot:0.1.1"
#import "cetz-figures.typ"
#import "cetz-elements.typ"
#import "acronyms.typ": acronyms
#import "glossary.typ": glossary

#show: unikn-thesis.with(
  title: "The shortest path to happiness: Efficiently finding beer-paths using Oracles",
  authors: (
    (name: "Jakob Sanowski", student-id: "1095786", course: "Bachelor Thesis", course-of-studies: "Informatik"),
  ),
  city: "Konstanz",
  type-of-thesis: "Bachelor Thesis",
  // displays the acronyms defined in the acronyms dictionary
  at-university: true,
  // if true the company name on the title page and the confidentiality statement are hidden
  bibliography: bibliography("sources.bib"),
  date: datetime.today(),
  language: "en",
  supervisor: (university: "Prof. Dr. Sabine Storandt"),
  university: "University",
  university-location: "Konstanz",
  university-short: "Uni KN",
  show-declaration-of-authorship: false,
  math-numbering: none,
  abstract: [
    This thesis analyses the concept of the in-path oracle introduced in the paper In-Path Oracles for Road Networks @Ghosh2023 for identifying Points of Interest (POIs) within a bounded detour from the shortest path between a source and destination in a road network.
    It defines essential concepts like shortest distance, detour, and in-path POIs.
    The study compares three algorithms: double Dijkstra, parallel dual Dijkstra, and an in-path oracle method that uses precomputed results to improve query times.

    The double Dijkstra algorithm runs two separate Dijkstra instances from the source and destination to find detours through POIs.
    The parallel dual Dijkstra runs these two Dijkstra instances simultaneously. The in-path oracle method leverages spatial coherence in road networks to precompute results, significantly reducing query times.

    Experiments were conducted on datasets from OpenStreetMap, specifically Konstanz and San Francisco, with varying detour limits and POI sampling rates.
    Results show that the in-path oracle method achieves higher throughput compared to the baseline dual Dijkstra, confirming its efficiency for large-scale applications.
    However, the oracle size was larger than expected, indicating a need for further optimisation and proof refinement.

    // The report concludes with insights into the practical feasibility of these algorithms and highlights areas for future work, including the need for a concrete proof of the oracle size bounds and further investigation into the impact of insufficient lemmas on algorithm performance.
  ],
)

// Edit this content to your liking

= Introduction

In graph theory and computer science, the problem of identifying paths that require visiting specific vertices, referred to as 'beer vertices,' presents a unique challenge.
This problem is particularly relevant in scenarios where paths must include certain checkpoints or resources, analogous to visiting a "beer store" in a network of roads.

The beer-path oracle @Ghosh2023 is a specialized data structure designed to efficiently answer queries related to beer paths, providing all beer vertices which are in-path for any two vertices.

This thesis delves into the performance of a beer-path oracle, exploring its efficiency, scalability, and practical applications.
We begin by outlining the theoretical foundations of the beer-path problem and discussing some problems which arose during analysis.
The core of this thesis focuses on the theoretical analysis of the oracle outlining the problems and shortcomings it has.

// #TODO: write more about the analysis
We present a comprehensive performance analysis, evaluating the oracle's response time and memory usage across different types of graphs.
Through empirical testing, we analyse the oracle's ability to handle graphs of different sizes and discuss the trade-offs between oracle size and query time.
Furthermore, we compare our beer-path oracle with a double dijkstra approach, underscoring its advantages and potential areas for improvement.

The findings of this thesis contribute to the ongoing research in graph algorithms and data structures, offering insights into the development of efficient pathfinding techniques under constrained conditions.


= Related Work

The _beer-path_ problem is closely related to shortest path finding on road networks.
Prior work in this area can be broadly categorized into oracle-based techniques, which use precomputation to answer queries quickly, node-importance-based methods, which leverage the graphical structure of the network, and on-the-way search approaches that find POIs with minimal deviation.
This section provides an overview of these approaches, drawing from the survey in .

== Path and Distance Oracles

The concept of a distance oracle was formally introduced by #cite(<Thorup2005>), which established a theoretical framework for trading off between preprocessing time, space, and query time.
Many subsequent approaches have been developed specifically for road networks, leveraging the principle of *spatial coherence*—the observation that spatially adjacent nodes share similar path characteristics.

For example, #cite(<Sankaranarayanan2009Path>) introduced a path oracle that encodes all $n^2$ shortest paths in $O(n)$ storage.
This was extended by #cite(<Sankaranarayanan2009Distance>) to create approximate distance oracles, also based on spatial coherence.
Other notable developments include City Distance Oracles (CDO), SPark and Distance Oracles (SPDO), and the Distance Oracle System (DOS), which are designed for high-throughput queries on large-scale road networks.
The _in-path oracle_ builds upon these foundations by applying the concept of spatial coherence to determine if groups of paths include a given POI.

== Node-Importance-Based Methods

In contrast to oracle-based techniques that rely on spatial properties, node-importance-based methods focus on the graph structure of the road network .
These approaches are based on the observation that certain nodes (e.g., major highway intersections) are more "important" and that most shortest paths will pass through at least one of them.
The general strategy is to rank nodes by a measure of importance and then pre-calculate the shortest path distances between these important nodes.
This precomputed information is then used to accelerate query processing at runtime.
The _in-path oracle_ differs significantly from these methods, as it aims to completely precompute away the graph information, relying solely on spatial data to answer queries.

== Detour and On-the-Way POI Queries

Several approaches directly address finding POIs along a route with a minimal detour. #cite(<Yoo2005>) introduced the "in-route nearest neighbor query," which finds the POI with the smallest deviation from a given shortest path.
Similarly, #cite(<Chen2009>) explored path nearest neighbor queries for dynamic road networks.

Other related problems include obstructed detour queries, where obstacles prevent straight-line navigation #cite(<Saha2018>), best point detour queries, which seek the detour with the lowest cost #cite(<Shang2010>), and finding routes that must include a stopover of a certain type #cite(<Nutanong2012>).
The work by #cite(<Ghosh2023>) is considered more fundamental because its oracle succinctly encodes the in-path status of a POI for every possible source-destination pair, allowing these more complex detour queries to be processed efficiently on top of it.

= Preliminaries

In this section we will establish some preliminary concepts.
We begin with the foundational concepts of graph theory and network paths, the describe detours and in-path POIs.
Lastly, we will introduce well-separated pairs (WSPs) and well-separated pair decomposition (WSPD).

== Graphs

Graphs represent relationships (_edges_) between objects (_vertices_).
They are the foundational structure for the algorithms discussed in this thesis.
In our case, graphs will be used to model road networks.

#definition("Graph")[
  An (undirected) graph is a tuple $G = (V, E)$, where
  - $V eq.def {v_1, dots, v_n}$ are the vertices.
  - $E subset.eq {(u,v) | u, v in V}$ are the edges.

  We denote $n eq.def |V|$ and $m eq.def |E|$.
]

For an edge $e = (u, v)$, we call $u$ and $v$ its endpoints and say they are incident to $e$ (and vice versa).
$u$ and $v$ are neighbours in the graph.
The set of all neighbours of a vertex is called its neighbourhood and, for a vertex $v$, we denote it with $cal(N)(v)$.

#definition("Network Paths")[
  Given a graph $G = (V, E)$, the sequence of vertices $Pi_G (s,t) = angle.l s = v_i, v_2, dots, t = v_k angle.r$ is called an _s-t-network-path_ of length $k-1$ if it connects two vertices $s$ and $t$ and
  - For all $v_i in Pi_G (s,t), v_i in V$.
  - For all $i in {2, dots, k}, (v_(i-1), v_i) in E$. We call $angle.l (v_1,v_2), dots, (v_(k-1), v_k) angle.r$ its _edge sequence_.

  Given a cost function, we call $c(Pi_G)$ its cost (see ).

  A path is said to be a _shortest s-t_-network-path if its cost is minimal among all _s-t_-paths. We then write $Pi_G^*$.
]

Note that even though the underlying graph is undirected, paths do specify a direction, i.e., $s$ and $t$ are not necessarily interchangeable.
Dijkstra’s algorithm @Dijkstra2022 is the most commonly used algorithm to find shortest network paths in graphs.
We will not discuss its details here, but refer to @cormen2022introduction.
When using Fibonacci Heaps @Fredman1987 as a priority queue, Dijkstra’s algorithm computes the shortest path in $cal(O) (n log n + m)$ time.

The most intuitive and ubiquitous cost function is the Euclidean cost function:

#definition("Euclidean Cost")[
  $
    c_("euclid") (p,q) = |overline(p q)| = sqrt((q_x - p_x)^2 + (q_y - p_y)^2)
  $
]

#definition("Shortest Network Distance")[
  Given source $s$ and destination $t$ nodes, $d_N (s, t)$ denotes the shortest network distance between $s$ and $t$, i.e., $c(Pi_G^*)$.
]

We define a _detour_ as the difference between a path and the shortest path:

#definition("Detour")[Given source $s$ and destination $t$ nodes, let $Pi_G(s, t)$ denote a simple path that is not necessarily the shortest.
  The detour $d_D$ of such a path is the difference in the network distance along $Pi_G (s, t)$ compared to $d_N (s,t)$.
  Furthermore, it is fairly trivial to see that the detour of any path is greater or equal to zero.]

We need to define a bounded _detour_ as follows:

#definition("Detour Bound")[
  A detour is bounded by a fraction $epsilon$ such that their total distance does not exceed $epsilon * d_N (s,t)$.
  For example, if $epsilon = 0.1$ a bounded detour can be up $10percent$ longer than the shortest path.]

Most crucially we define a POI to be in path when:

#definition("In-Path POI")[A POI is said to be _in-path_ if there exists a detour bounded by $epsilon$ which passes through said POI.]


== Well-Separated Pair Decomposition

Given a point set $A$ then $r$ denotes the radius of the hypersphere containing all points in $A$.
The _minimum distance_ of two point sets $A$ and $B$ is the distance between the hyperspheres containing them.

#definition("Well-Separated Pair")[
  Two sets of points are considered _well-separated_ if the _minimum distance_ between $A$ and $B$ is at least $s dot r$, where $s > 0$.
  $s$ is the _separation factor_ and $r$ is the larger radius of the two sets. Such a pair is termed a _well-separated pair_ (WSP).
]

#cetz-figures.fig_wsp(
  caption: [$A$ and $B$ are well-separated if the distance between them is larger than $s r$.],
) <fig_wsp>

#definition("Well-Separated Pair Decomposition")[
  A _well-separated pair decomposition_ (WSPD) of a point set $S$ is a set of WSPs such that $forall u, v in S, u != v$, there is exactly one WSP $(A, B)$ with $u in A, v in B$.
]

One possible WSPD would be pairs of singleton element subsets $(u,v) forall u, v in S, u != v$ containing $n dot (n -1)$ pairs.
It has been proven one can always construct a WSPD of size $O(s^d n)$ @callahan1995dealing.

Such a WSPD of $S$ can be constructed by first constructing a PR quadtree $T$ on $S$.
The decomposition of $S$ into WSPs using $T$ is called a _realization_ on $T$, i.e., the subsets $A_i, B_i$ of $S$ forming a WSP $(A_i, B_i)$ correspond to nodes of $T$.
Starting with the pair $(T,T)$ corresponding to the root of $T$ we check for each pair $(A, B)$ if it is separated with respect to $s$.
If so it is reported as WSP.
Otherwise, we pair each child of $A$ with each child of $B$ in $T$ and repeat the process until all leafs of $T$ are covered.

== Problem Definition

We are given a road network $G$, set $P$ of $m$ POIs, and a detour bound $epsilon$.
A driver travels from source $s$ and destination $t$, we want to find the set of pois in $p$ that are “in-path”
under the conditions specified.

= The In-Path Oracle <algos>

In a recent paper @Ghosh2023, the authors introduce a method for computing a _in-path_ oracle.
They adapt the distance oracle proposed by @Sankaranarayanan2009Distance to the _in-path_ problem by introducing an _in-path_ and _not-in-path_ property for block pairs.
They claim the precomputation for city-sized road networks is in the tens of minutes with linear memory consumption.

In this section we will look at their methodology and rationale behind it.
We will explain their algorithms and try to fill crucial gaps in their description.
First, we will outline a baseline method for solving the _in-path_ problem.
Then, we give some background information necessary to understand the _in-path_ oracle.
Last, we will discuss the effectiveness of their method and give some possible improvements.

== Double Dijkstra

// #TODO: Description is enough
The double Dijkstra is a Dijkstra variant for finding detours passing through one $p in P$.
We use two separate instances of Dijkstra starting from the start $s$ and end $t$ node respectively.
The input for both instances are all POIs from $P$ and $t$ for the instance starting from $s$.
We combine the result of both instances by adding the costs from both instances for every $p in P$ together.
It is important to note for the instance starting from $t$ we traverse the edges backwards.

== Parallel Dual Dijkstra

#cite(<Ghosh2023>, form: "prose") proposed the dual Dijkstra algorithm for finding POIs within a specified detour tolerance limit $epsilon$ which we developed a parallel version of.
In order to parallelize the algorithm we run two Dijkstra at the same time starting from the source $s$ and destination $t$ similar to the double Dijkstra.

@par-dual-dijkstra describes the algorithm of both instances.
Each instance uses its own a priority queue $Q$ over the distance to its respective start node.
Every node $n$ additionally holds the distance to the start and a label which can be accessed with the functions $d(n)$ and $l(n)$.

At the core of this algorithm is the shared data structure #smallcaps[Visited].
This data structure holds all nodes visited by both Dijkstra instances together with a label indicating which instance found the node and the distance to the start node $s$ or $t$ respectively.
The key of this algorithm is in Line 10 where we add the two distances together.
If this node $n in P$ we mark it as $bb("POI")$ so it gets added to the result.

#algorithm-figure(
  "Dual Dijkstra",
  {
    While(
      [_!Q.empty()_],
      {
        If(
          $l(n) == bb("POI")$,
          {
            Line([_result.add(n)_])
            Line([continue])
          },
        )
        If(
          [#smallcaps[Visited]\(_n, l(n)_\)],
          {
            Line([continue])
          },
        )
        If(
          [$n_r arrow.l$ #smallcaps[Visited]\(_n, l(n).inverse()_\)],
          {
            Assign($d'$, $d(n) + d(n_r)$)
            Assign([_n.distance_], $d'$)
            Assign($d_N$, [_min_$(d_N, d' * (1 + epsilon))$])
            If(
              $n in P$,
              {
                Line([_Q.insert(n.label(#math.bb("POI")))_])
              },
            )
            Line([#smallcaps[Visited]_\.insert(n)_])
            For(
              $"neighbour" v_i "of" n$,
              {
                Line([_Q.insert($v_i$.label(l(n)))_])
              },
            )
          },
        )
      },
    )
    Return([_result_])
  },
) <par-dual-dijkstra>

== Beer-Path Oracle <beer-path-oracle>

The beer-path oracle proposed by #cite(<Ghosh2023>, form: "prose") aims to reduce query times using precomputed results.
It uses the _spatial coherence_ @Sankaranarayanan2005 property in road networks which observes similar characteristics for nodes spatially adjacent to each other.
Or more precisely the coherence between the shortest paths and distances between nodes and their spatial locations @Sankaranarayanan2005 @Sankaranarayanan2009Distance.
We know for a set of source nodes $A$ and destination nodes $B$ they might share the same shortest paths if $A$ and $B$ are sufficiently far apart and the nodes contained in $A$ and $B$ are close together.
This enables determining if a POI is in-path with respect to this group of nodes opposed to single pairs of nodes.

The focus here is maximizing the throughput where one can answer millions of in-path queries a second using a single machine.

This approach though is not able to find multiple POIs one might want to visit without exceeding the detour bound.
It is expected that the user only wants to visit one of the presented POIs.
Such examples include coffee shops, restaurants, gas stations,
vaccination clinics, etc.


=== In-Path Property

#cetz-figures.fig_in-path <figure-in-path>

In order to define the _in-path_ property for a set of source nodes $A$ and a set of destination nodes $B$ these sets are restricted to be inside a bounding box containing all nodes.
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

Note that the condition $d_N (a_r, b_r) - (r_a^F + r_b^B) > 0$ is omitted by #cite(<Ghosh2023>, form: "prose") but is necessary because $d_N (a_r, b_r)$ can be 0 in which case $d_N (a_r, b_r) - (r_a^F + r_b^B) < 0$ and thus the condition would suddenly be satisfied if $d_N (a_r, b_r)$ is smaller than some specific value.
Even $d_N (a_r, b_r) > 0$ would not be enough because $d_N (a_r, b_r) > (r_a^F + r_b^B)$ still isn't guaranteed.

#lemma("Not In-Path Property")[
  A block pair $(A,B)$ is not _in-path_ if the following condition is satisfied:
  $ (d_N (a_r,p) + d_N (p, b_r) - (r_a^B + r_b^F)) / (d_N (a_r, b_r) + (r_a^B + r_b^F)) -1 >= epsilon $
]

#proof[
  For any given nodes $s$, $t$ in $A, B$, respectively, $d_N (s,t)$ is at most $d_N (a_r, b_r) + (r_a^B + r_b^F)$ (see @lemma-Longest-Shortest-Path).
  Considering the path $s -> a_r -> p -> b_r -> t$ it has a length of at least $d_N (a_r, p) + d_N (p, b_r) - (r_a^B + r_b^F)$.
  We get the following inequality in order for all possible paths in $A,B$ to not be _in-path_ to $p$:
  $
    d_N (a_r, p) + d_N (p, b_r) - (r_a^B + r_b^F) >= (d_N (a_r, b_r) + (r_a^B + r_b^F)) dot (1 + epsilon) \
    (d_N (a_r, p) + d_N (p, b_r) - (r_a^B + r_b^F)) / (d_N (a_r, b_r) + (r_a^B + r_b^F)) - 1 >= epsilon
  $
]

#lemma("In-Path Parent")[
  A block pair $(A, B)$ is _in-path_ if all its children are _in-path_
]

#proof[
  For any given nodes $s, t$ in $A, B$ respectively we find a child block pair $(A', B')$ with $s in A'$ and $t in B'$.
  Because all child block pairs of $(A, B)$ are _in-path_, $s, t$ are _in-path_ and thus $(A, B)$ has to be _in-path_.
]

// TODO: Describe algorithm. Note the many calls to dijkstra

@algo-in-path-oracle describes the algorithm proposed by #cite(<Ghosh2023>, form: "prose") for computing the _in-path_ oracle.

#show: style-algorithm
#algorithm-figure(
  "In-Path Oracle for a given POI",
  {
    Assign[$R$][root block of the road network]
    Assign[$#math.italic([result])$][$emptyset$]
    Assign[$Q$][$(R,R)$]
    While(
      $#math.italic("!Q.empty()")$,
      {
        Assign[$(A,B)$][$#math.italic("Q.pop_front()")$]
        Assign[$a_r,b_r$][random node from $A, B$, respectively]
        Assign[$#math.italic("values")$][Compute $d_N (a_r,b_r), d_N (a_r,p), d_N (p,b_r), r_a^F, r_a^B, r_b^F, r_b^B$]

        If(
          $#math.italic("values.in-path()")$,
          {
            [$#math.italic("result.add((A,B))")$]
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

The algorithm takes a road network as input and $R$ denotes the root block of a quadtree on the spatial positions of the nodes.
$Q$ is a queue holding the block pairs.
We initialize $Q$ with the block pair $(R,R)$.
The algorithm runs until there are no more block pairs in $Q$.

In each iteration the front block pair $(A, B)$ is retrieved from $Q$.
The representants $a_r$ and $b_r$ are randomly chosen from $A$ and $B$ respectively.
The algorithm proceeds to compute the shortest network distance between the two representants $d_N (a_r, b_r)$ as well as the shortest network distance while passing through $p$, i.e., $d_N (a_r, p)$ and $d_N (p, b_r)$.
It also computes the radii of $A$ and $B$ $r_a^F, r_a^B, r_b^F, r_b^B$.
Because computing the radii for a block $A$ requires to compute Dijkstra from $a_r$ to every other node in $A$ we often end up computing Dijkstra multiple times for the same pair of nodes.
Using a cache greatly improves computation time for this step.
Lastly, the algorithm checks if $(A,B)$ is either _in-path_ or _not-in-path_.
If $(A,B)$ is _in-path_ it is added to the result, if it is _not-in-path_ the algorithm just continues to the next iteration.
If neither is the case $A$ and $B$ are split into their 4 children blocks and each child of $A$ is paired with each child of $B$.
The resulting block pairs are inserted into $Q$.

#cite(<Ghosh2023>, form: "prose") claim the size of the _in-path_ oracle is $O(1 / epsilon^2 n)$.
Their proof references the arguments in @Sankaranarayanan2009Distance.
In order to provide some context we will give an overview over these arguments.

=== Well-Separated Pair Decomposition

The distance distortion is the ratio of the network distance to the spatial distance between two vertices.
One can define a minimum and maximum distortion $gamma_L, gamma_H$ for a spatial network such that
$
  gamma_L <= (d_G (u,v)) / (d_S (u,v)) <= gamma_H; gamma_L. gamma_H > 0.
$


#lemma("Packing Lemma")[
  Considering an arbitrary point set $A$ then a block with side length $2 r$ encloses all points in $A$.
  The total number of blocks with side length $2 r$ which are not _well-separated_ from $A$ is bounded by the number of blocks contained within a hypersphere of radius $(2s + 1)r$ centred at $A$, which contains a maximum of $O(s^d)$ blocks.
]

#proof[
  It is trivial to see that the total number of blocks contained within a hypersphere of radius $(2s + 1)r$ is upper bound by $((2s + 1)r)^d$.
  Because $r$ is a constant we get $O(s^d)$.
]


#cetz-figures.fig_packing-lemma(caption: [Visualization of @lemma-Packing-Lemma])


Using the packing lemma we get a size of $O(s^d n)$ for a WSPD since a PR quadtree has $O(n)$ inner nodes and each inner node can produce a maximum of $O(s^d)$ WSPs.

For a WSPD build using the network distance we can bound $r'$ by
$
  r' <= gamma_H r.
$
The effective separation factor $s'$ is $s gamma_H$.
Therefore, the size of the WSPD is $O((s)^d n)$ and because $gamma_H$ is a constant independent of $n$.

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
  #image("assets/R-tree.svg", height: 300pt)
] <fig-r-tree>

The performance of an _R-Tree_ greatly depends on the overlap of the bounding boxes in the tree.
Generally less overlap leads to better performance.
For this reason the insertion strategy is crucial for achieving good performance.
_R\*-Trees_ try to minimize the overlap by employing insertion strategies which take this into account.
This improves pruning performance, allowing exclusion of whole pages form search more often.
The key for achieving this is based on the observation that _R-Trees_ are highly susceptible to the order in which their entries are inserted.
For this reason the _R\*-Tree_ performs reinsertion of entries to “find” a better suited place in the tree.

In the case of a node overflowing a portion of its entries are removed and reinserted into tree.
To avoid infinite reinsertion, this may only be performed once per level of the tree.

== Limitations

The approach presented in @beer-path-oracle has some shortcomings especially in its space consumption. In this section we will look at some possible reasons for these shortcomings.

The biggest shortcoming of the _in-path_ oracle is the space consumption.
We found the oracle to be very large even on relatively small instances.
Furthermore, it was not possible to test instances of similar size to the instances used by @Ghosh2023.
This bakes the question for the cause of the large size of the oracle.

=== Theoretical Shortcomings

We found the proof for the space complexity to be insufficient for proofing a size of the oracle of $cal(O)((1 / epsilon)^2 n)$.
In the following we will show why this proof does not work.

#definition("Radius")[
  Let $r$ be the average of $r_a^F, r_a^B, r_b^F, r_b^B$ such that $4r = r_a^F+ r_a^B+ r_b^F+ r_b^B$.
]

We can use $r$ to get an upper bound for the average over all the specific radii which should give us an idea how large the block pairs can be in relation to their distance.

#lemma("In-Path Radius Upper Bound")[
  With $d_D$ denoting the detour through $p$ for any block pair $(A, B)$ to be _in-path_ the average radius is bound by:
  $
    r <= (d_N (a_r,b_r) epsilon - d_D) / (4 + 2 epsilon)
  $
]

#proof[
  Using @lemma-In-Path-Property gives us:
  $
    (d_N (a_r,b_r) + d_D + 2r) / (d_N (a_r,b_r) - 2r) & <= 1 + epsilon \
    d_N (a_r,b_r) + d_D + 2r & <= (1 + epsilon) (d_N (a_r,b_r) - 2r) \
    4r & <= d_N (a_r,b_r) epsilon - 2r epsilon - d_D \
    4r + 2r epsilon & <= d_N (a_r,b_r) epsilon - d_D \
    r(4 + 2 epsilon) & <= d_N (a_r,b_r) epsilon - d_D \
    r & <= (d_N (a_r,b_r) epsilon - d_D) / (4 + 2 epsilon)
  $
]

We can see $r$ can be at most $1 / 4$ of $d_N (a_r,b_r) epsilon - d_D$ for a block pair to be _in-path_.
This is especially bad for small $epsilon$ because then $d_N (a_r,b_r) epsilon$ is small which in turn causes $r$ to be a small fraction of $d_N (a_r,b_r)$.
Moreover, $d_D$ is subtracted from $d_N (a_r,b_r) epsilon$ causing $r$ to have to be even smaller or even zero.

#lemma("Not In-Path Radius Upper Bound")[
  With $d_D$ denoting the detour through $p$ for any block pair $(A, B)$ to be not _in-path_ the average radius is bound by:
  $
    r <= (d_D - d_N (s,t) epsilon) / (4 + 2 epsilon)
  $
]

#proof[
  Using @lemma-Not-In-Path-Property gives us:
  $
    (d_N (s,t) + d_D - 2r) / (d_N (s,t) + 2r) & >= 1 + epsilon \
    d_N (s,t) + d_D - 2r & >= (1 + epsilon) (d_N (s,t) + 2r) \
    4r + 2r epsilon & <= d_D - d_N (s,t) epsilon \
    r & <= (d_D - d_N (s,t) epsilon) / (4 + 2 epsilon)
  $
]

For a block pair to be _not-in-path_ $r$ is primarily bound by $d_D$ which makes sense because a large detour increases the difference to the detour limit and thus increases the size a block can have without containing a node which can have a detour within the limit. // ????

Using @lemma-In-Path-Radius-Upper-Bound and @lemma-Not-In-Path-Radius-Upper-Bound we can find a bound for $d_N (s,t)$ where it is neither _in-path_ nor _not-in-path_ or in other words where a block pair $(A, B)$ is not well-separated.

#lemma("Not Well-Separated Block")[
  A block pair $(A, B)$ is not well-separated when
  $
    (-r(4 + 2 epsilon) + d_D) / epsilon < d_N (s,t) < (r(4 + 2 epsilon) + d_D) / epsilon
  $
]

#proof[
  Solving @lemma-In-Path-Radius-Upper-Bound and @lemma-Not-In-Path-Radius-Upper-Bound for $d_N (s,t)$ gives us
  $
    d_N (s,t) >= (r(4 + 2 epsilon) + d_D) / epsilon
  $
  and
  $
    d_N (s,t) <= (-r(4 + 2 epsilon) + d_D) / epsilon
  $

  Using their negations we get:
  $
    (-r(4 + 2 epsilon) + d_D) / epsilon < d_N (s,t) < (r(4 + 2 epsilon) + d_D) / epsilon
  $
]

We can see for a block pair $(A, B)$ to be a WSP is dependent on $d_D$.
This poses a problem because we can no longer use the spacial coherence argument like #cite(<Ghosh2023>, form: "prose") suggest.
@fig_no-spatial-coherence shows how only the relation to the _POI_ is relevant for a block pair to be a WSP.
It is not possible any more to define a hypersphere around a block which contains all blocks not well-separated from it, so the packing lemma does not apply any more.
We therefore can not get an upper bound for the total number of blocks which are not well-separated from any given block and thus cannot guarantee the size of the oracle to be $O((1 / epsilon)^d n)$.

#cetz-figures.fig_no-spatial-coherence <fig_no-spatial-coherence>


=== Practical Worst Cases

In order to get a better understanding of the performance of @algo-in-path-oracle we build a tool to visualize the results produced by the algorithm.
It enables us to look at the concrete values for any block pair as well as the paths leading to these values (see @figure-tool-showcase).
The tool also allows us to have a look at intermediate results occurring during the execution of the algorithm.
We could identify multiple cases proofing to be unfavourably for the algorithm.

#figure(
  caption: [A block pair is visualized in pink. The green dots show the representant of the block. The yellow dot shows the POI associated with the block pair. The shortest path is green. The detour is the red path. The blue paths are the radii of the blocks.],
)[
  #image("assets/tool-showcase.png")
] <figure-tool-showcase>

Road networks regularly contain nodes which are very close in Euclidean space but have a relatively high road network distance.
This case is very common on the border between different suburbs because they are often self-contained networks with only one or two access roads with no roads connecting the suburbs.
Another reason can be some kind of obstacle having to go around.

#figure(
  caption: [In order to reach the point on the other side of the train station, a relatively large detour is taken compared to the euclidean distance.],
)[
  #image("assets/large-radius.png", height: 300pt)
]


One-Way streets tend cause larger radii and thus the blocks to be smaller.
As we can see in @figure-one-way-radii to reach some nodes inside the block we have to take a significantly longer route due to one-way street.
This has the effect of the radii being very long in relation to the size of the block.
Furthermore, it can require blocks to be split until only one node is contained in a block because we always have to take the long route to reach other nodes on the one-way street.

#figure(caption: [One-Way streets increase the radii (blue) because having to go around])[
  #image("assets/one-way-street-radii.png", height: 300pt)
] <figure-one-way-radii>

#cetz-figures.fig_one-way-street <figure-one-way-street>

@figure-one-way-street illustrates this problem.
When $p_2$ is the representant for this block we have to take a really long route to reach $p_1$.

As established in @lemma-Not-Well-Separated-Block, for a block pair to be well-separated is independent of $d_S (a_r,b_r)$.
@fig_well-separated_blocks shows a block pair which is _not-in-path_ and thus a WSP.
@fig_not-well-separated_blocks shows the same block pair but here it is not a WSP because the detour is smaller and therefore it is not possible to decide if the block pair is _in-path_ or _not-in-path_.
This is the reason it is not possible to use the argument of the packing lemma.

#grid(
  columns: (1fr, 1fr),
  column-gutter: 5pt,
  [#figure(
      caption: [Not-in-path block pair and thus a WSP.],
      image("assets/well-separated_blocks.png"),
    ) <fig_well-separated_blocks>],
  [#figure(
      caption: [The same block pair not a WSP for a different POI.],
      image("assets/not-well-separated_blocks.png"),
    ) <fig_not-well-separated_blocks>],
)

== Improvements


=== Merge

On real world data we could observe for many block pair all their children either all being _in-path_ or _not-in-path_.
Therefore, using @lemma-In-Path-Parent we can mark a block pair as _in-path_ if all its children are _in-path_.

@algo_merged-in-path-oracle describes an algorithm utilising this observation.
The algorithm takes a road network as input and $R$ denotes the root block of a quadtree on the spatial positions of the nodes.
The function *process_block_pair()* lies at the heart of the algorithm.
It takes a block pair as input and returns if it is _in-path_ or not.
Note that *process_block_pair()* returns _false_ if a block pair is truly _not-in-path_ or had to be split.
The initial block pair is the root block paired with it self.

Similar to @algo-in-path-oracle we select $a_r$ and $b_r$ at random from $A$ and $B$ respectively and compute the shortest distance between $a_r$ and $b_r$, the distance of the shortest path passing through $p$ as well as the radii of $A$ and $B$.
If we find $(A, B)$ to be _in-path_ or _not-in-path_ we simply return _true_ or _false_ respectively.
If neither is the case we split $A$ and $B$ into their 4 children blocks pair each child of $A$ with each child of $B$.
We proceed to call *process_block_pair()* on each resulting block pair.

Using @lemma-In-Path-Parent we can simply report the current block pair as _in-path_ by returning _true_ if we find all child block pairs to be _in-path_ as well.
Otherwise we add all child block pairs which are _in-path_ to the result and return _false_.

#algorithm-figure(
  "Merged In-Path Oracle for a given POI",
  {
    let process_block = Fn.with("process_block_pair")
    Assign[$R$][bounding box of the road network]
    Assign[$#math.italic([result])$][$emptyset$]


    Line(process_block[$(R, R)$])

    Function(
      "process_block_pair",
      ($(A, B)$),
      {
        Assign[$a_r,b_r$][random node from $A, B$, respectively]
        Assign[$#math.italic("values")$][Compute $d_n (a_r,b_r), d_N (a_r,p), d_N (p,b_r), r_a^F, r_a^B, r_b^F, r_b^B$]

        If(
          $#math.italic("values.in-path()")$,
          {
            Return[true]
          },
        )
        If(
          $#math.italic("values.not-in-path()")$,
          {
            Return[false]
          },
        )

        Assign[_children_][Subdivide $A$ and $B$ into 4 children blocks. Discard empty children blocks.]

        For(
          [_child_ in _children_],
          {
            process_block[_child_]
          },
        )

        If(
          [all children in-path],
          {
            Return[true]
          },
        )


        For(
          [_child_ in _children_],
          {
            If(
              [_child_ is in-path],
              [_result.add((A, B))_],
            )
          },
        )

        Return[false]
      },
    )
  },
) <algo_merged-in-path-oracle>

=== Ceter Representant

Another possible improvement would be to try and minimize $r$.
This directly follows from @lemma-Not-Well-Separated-Block.
A lower $r$ decreases the range for $d_N (a_r, b_r)$ and require the block pair to be split and thus increasing the possibility for it to be either _in-path_ or _not-in-path_.

$r$ can be minimized by simply choosing the node closest to all other nodes in a block.
Because we need to compute the distance between each node in a block to get $r^F$ and $r^B$ it does not cost any extra computing time.


= Implementation Details

This section describes some of the technical details and challenges in our implementation.
Not every aspect is described in detail, but rather we highlight the most interesting aspects.

The implementation accompanying this thesis was written in Rust, a compiled general-purpose programming language. The decision to use Rust was based on its performance, combined with type and memory safety.
We use its in-house tool cargo to manage dependencies on third-party libraries #footnote[These are called _crates_ in the Rust ecosystem].
All of our code has been written in safe Rust to avoid any memory leaks or undefined behavior.

==== Geospatial Primitives

The `geo` library provides data types for geospatial primitives like points, lines, polygons or line strings.
Even though we only use the point type for storing the spatial positions of vertices, the widespread use of the `geo` library by other libraries provides a good base for interoperability.
Furthermore, `geo` provides basic spatial algorithms like geodesic distance calculations.

== Graph Data Structure

Because we need to be able to work on the graph as well as the spatial relation of the vertices we build a custom data structure combining these two.
As a base we used a compressed sparse row @Bulu2009 graph representation.
The positions of the vertices are stored in an _R\*-Tree_ provided by the `rstar` library and linked to the graph vertices.
This enables fast retrieval of all vertices inside a given area.

== File Handling

The road network is provided as _Geojson_.
It contains geometry objects with tags describing the type of geometry (e.g. streets).
To read a road network into memory we use the `geo-zero` library.
It enables us to filter out unwanted geometry like foot-paths.
After creating the graph data structure we find the biggest strongly-connected-component and delete all other components.
This is necessary because our algorithms do not handle the case of two unconnected vertices.

To write the graph and oracle to external memory, and to load it for later invocation of the algorithms, we use the `serde` library.
It provides extensive support to serialize and deserialize data from internal memory into a number of file formats.
This makes it possible to have to perform the expensive computation of the oracle only once and load it from disk for later analysis.

= Experimental Evaluation

The experiments were performed on an AMD Ryzen 5 5600X with 6 cores and 12 threads at 4.651 GHz and 16 GB of RAM.

== Dataset

The road networks used for evaluation were obtained from OpenStreetMap and sanitized of foot-paths to only include one edge per street. We used two datasets in our evaluation, Konstanz with 2282 nodes and 4377 edges and San Francisco with 95092 nodes and 172256 edges. The weight of each directed edge denotes the travel distance between two nodes. Note that _chains_ (or _ways_) are not simplified.

=== Comparative Experiments

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
Unfortunately we could not compute an _in-path oracle_ for the San Francisco dataset in reasonable amount of time.

=== Varying Detour Limits

To measure the impact of the detour limit on the oracle size we varied the detour limit from 0.05 to 5.
The test were performed on the Konstanz data set consisting of 2282 nodes and 4377 edges.
As we can see in @fig-oracle-size the oracle size is roughly bell-curve shaped, which makes sense when looking at @lemma-In-Path-Property and @lemma-Not-In-Path-Property.
When $epsilon$ is very small @lemma-Not-In-Path-Property is more easily satisfied.
Similarly, when $epsilon$ is very big @lemma-In-Path-Property is satisfied for bigger blocks.
It is important to note #cite(<Ghosh2023>, form: "prose") report much smaller sizes for a graph of this size.
We only get similar results when applying the merging step though our results are still slightly higher than #cite(<Ghosh2023>, form: "prose").
For a graph with 5000 nodes they report an oracle size of a bit more than 100,000 compared to the 3,010,095 (see @fig-oracle-size) we found for a graph with 2248 nodes.


#figure(
  caption: [Size of the oracle for different $epsilon$.],
  cetz.canvas({
    import cetz.draw: *
    import cetz-plot: *

    let base = (
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

    let merged = (
      (0.05, 3205),
      (0.1, 31186),
      (0.2, 92320),
      (0.25, 104941),
      (0.3, 115737),
      (0.4, 130053),
      (0.5, 130703),
      (0.75, 135505),
      (1, 127132),
      (2, 101618),
      (3, 80664),
      (4, 67384),
      (5, 58690),
    )

    let x-tic-list = base
      .enumerate()
      .map(((i, t)) => {
        (i, t.at(0))
      })

    let data-mapped-base = base
      .enumerate()
      .map(((i, t)) => {
        (i, t.at(1))
      })
    let data-mapped-merged = merged
      .enumerate()
      .map(((i, t)) => {
        (i, t.at(1))
      })
    let x-inset = 0.5

    plot.plot(
      size: (10, 10),
      x-label: [$epsilon$],
      x-min: -x-inset,
      x-max: data-mapped-base.len() + x-inset - 1,
      y-label: "# of block-pairs",
      y-min: 0,
      y-max: 4800000,
      x-ticks: x-tic-list,
      x-tick-step: none,
      plot-style: (stroke: kn_seeblau, fill: kn_seeblau35),
      mark-style: (stroke: kn_seeblau, fill: kn_seeblau35),
      legend: (9.8, 9.8),
      legend-anchor: "north-east",
      {
        plot.add(data-mapped-base, mark: "o")
        plot.add-legend([no-merge])
        plot.add(
          data-mapped-merged,
          style: (stroke: kn_bordeaux, fill: kn_bordeaux35),
          mark: "o",
          mark-style: (stroke: kn_bordeaux, fill: kn_bordeaux35),
        )
        plot.add-legend(
          [merge],
          preview: () => {
            line((0, 0.5), (1, 0.5), stroke: kn_bordeaux)
          },
        )
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
We will ignore the results of the parallel dual Dijkstra moving forward because it always performed worse than the normal dual Dijkstra.

#figure(
  caption: [Throughput of the dual Dijkstra and Oracle for different sampling rates.],
  cetz.canvas({
    import cetz.draw: *
    import cetz-plot: *

    let oracle = (
      (0.0001, 58314),
      (0.0005, 57903),
      (0.001, 56733),
      (0.005, 56433),
      (0.01, 6366.6),
      (0.05, 2074.6),
      (0.1, 776.77),
      (0.5, 183.21),
    )
    let dijkstra = (
      (0.0001, 28.197),
      (0.0005, 29.551),
      (0.001, 27.656),
      (0.005, 28.307),
      (0.01, 28.350),
      (0.05, 28.990),
      (0.1, 28.498),
      (0.5, 28.133),
    )

    let x-tick-list(data) = {
      data
        .enumerate()
        .map(((i, t)) => {
          (i, t.at(0))
        })
    }
    //
    //
    let map-data(data) = {
      data
        .enumerate()
        .map(((i, t)) => {
          (i, t.at(1))
        })
    }

    let x-inset = 0.5

    plot.plot(
      name: "o-size",
      size: (10, 10),
      x-label: [POI sampling rate],
      x-min: -x-inset,
      x-max: x-tick-list(oracle).len() + x-inset - 1,
      x-ticks: x-tick-list(oracle),
      x-tick-step: none,
      y-label: [K queries/second],
      y-mode: "log",
      y-min: 5,
      y-max: 100000,
      y-ticks: (10, 50, 100, 500, 1000, 5000, 10000, 50000, 100000),
      y-tick-step: none,
      plot-style: (stroke: kn_seeblau, fill: kn_seeblau35),
      mark-style: (stroke: kn_seeblau, fill: kn_seeblau35),
      legend: (9.8, 9.8),
      legend-anchor: "north-east",
      {
        plot.add(map-data(oracle), mark: "o")
        plot.add-legend([Oracle])

        plot.add(
          map-data(dijkstra),
          style: (stroke: kn_bordeaux, fill: kn_bordeaux35),
          mark: "o",
          mark-style: (stroke: kn_bordeaux, fill: kn_bordeaux35),
        )
        plot.add-legend(
          [Dijkstra],
          preview: () => {
            line((0, 0.5), (1, 0.5), stroke: kn_bordeaux)
          },
        )
      },
    )
  }),
) <fig-throughput>



We observe a constant throughput of about 28,000 _in-path_ queries/second for the dual Dijkstra on most POI sampling rates running on only one single thread. This is due to the search space being dependent on $epsilon$ and thus not changing for different sampling rates.
As expected the _in-path_ oracle has a much higher throughput than the dual Dijkstra.
@fig-throughput clearly shows we get more than 100,000 _in-path_ queries per second for all sampling rates.
This confirms the findings of #cite(<Ghosh2023>, form: "prose").

= Conclusions and Future Work

In this work we examined the _beer-path_ problem and its intricacies and difficulties associated with building an oracle for it. We devoted particularly attention to a method building on WSPDs and its use for distance oracles @Sankaranarayanan2009Distance proposed by #cite(<Ghosh2023>, form: "prose").

Although the idea of using a similar approach as distance oracles the practicality leaves more to be desired.

We could somewhat verify the results with regard to the throughput on small instances.
On bigger instances the time to compute the oracle is to long to be practically feasible which stays in contrast to the 30 minutes claimed by #cite(<Ghosh2023>, form: "prose").
The oracle size though, we find to be bigger by a factor of more than 10 and also exceeds the upper bound they presented which could be why the compute time is so high.
This obviously has an impact on the throughput because of the massive increase in search space (see @fig-throughput).
We were able to show, that the proof provided for the size of the oracle is not sufficient.
Because the size of the oracle exceeded the bound presented by #cite(<Ghosh2023>, form: "prose") further work should be conducted to provide a concrete proof.

Another contribution of ours is an improvement to the algorithm creating the oracle.
We identified the oracle to be more fine grain than necessary.
On many occasions a block pair got split even though all children turned out to be either all _in-path_ or _not-in-path_.
In order to reduce the required space for storing the oracle we only save the parent block pair.
This suggests the _in-path_ property can be possibly improved to prevent this split.

Furthermore, we find @lemma-In-Path-Property to be insufficient.
Precisely the term $d_N (a_r, b_r) - (r_a^F + r_b^B)$ can be less than 0 because $d_N (a_r, b_r) > (r_a^F + r_b^B)$ is not guaranteed which is why the isolation of $epsilon$ is not possible.

Looking at the findings of this work we can see the potential of the _in-path oracle_ @Ghosh2023 though it lacks details to be easily reproducible.
Especially with regard to the scalability we could not confirm the claims they made and find some proofs insufficient.
