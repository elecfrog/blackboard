https://kowshik.github.io/JPregel/pregel_paper.pdf

Pregel: A System for Large-Scale Graph Processing

# 1. INTRODUCTION

Pregel程序的高级组织受到Valiant的批量同步并行模型的启发。
Pregel计算由一系列称为supersteps的迭代组成。
在每个superstep中，框架以概念上的并行方式为每个顶点调用用户定义的函数。

该函数指定单个顶点V和单个superstep S的行为。它可以读取在superstep S-1中发送给V的消息，发送将在superstep S+1中接收的消息给其他顶点，并修改V及其出边的状态。消息通常沿着出边发送，但可以发送到任何已知标识符的顶点。

这种以顶点为中心的方法让人想起MapReduce，因为用户专注于本地动作，独立处理每个条目，系统组合这些动作以将计算提升到大型数据集。该模型在设计上非常适合分布式实现：它不暴露任何检测superstep内执行顺序的机制，所有通信都是从superstep S到superstep S+1。

该模型的同步性使得在实现算法时更容易推理程序语义，并确保Pregel程序本质上避免了异步系统中常见的死锁和数据竞争。原则上，如果有足够的并行slack，Pregel程序的性能应该与异步系统具有竞争力。因为典型的图计算有很多比机器更多的顶点，所以应该能够平衡机器负载，使superstep之间的同步不会增加过多的延迟。

# MODEL OF COMPUTATION

Pregel 计算的输入是一个有向图。
图中每个顶点都由一个唯一的字符串 ID 标识，并关联一个可修改的用户自定义值。
有向边则归属于其源顶点（Source Vertex），每条边包含一个可修改的用户自定义值以及一个目标顶点 ID。

Pregel 计算通常由三部分组成：
1. 输入阶段，也就是图的初始化；
2. 随后是一系列由全局同步点分隔开的 superstep，直到算法结束；
3. 最后是输出阶段。

在每个 superstep 中，顶点会并行执行计算。
每个顶点都运行同一个用户自定义函数，用来表达具体算法的逻辑。
顶点可以修改自身状态或其出边的状态，接收上一轮 superstep 发送给它的消息，向其他顶点发送消息以便在下一轮 superstep 中接收，甚至可以改变图的拓扑结构。
在这个模型中，边并不是一等公民，因此不会参与独立计算。

算法的终止依赖于每个顶点“投票停止”。在 superstep 0 中，所有顶点都处于活动状态，参与每一轮 superstep 的计算。顶点可以通过“投票停止”将自己置为非活动状态。这表示它暂时没有更多工作要做，除非被外部触发；在后续的 superstep 中，Pregel 框架不会再执行该顶点，除非它接收到消息。若顶点因收到消息而重新激活，就必须再次显式地投票停止。只有当所有顶点都同时处于非活动状态，并且没有消息在传递时，整个算法才会终止。这个简单的状态机如图 1 所示。

Pregel 程序的输出是由顶点显式输出的值组成的集合。这个输出通常是一个与输入同构的有向图，但这并不是系统的必要特征，因为计算过程中顶点和边都可以被添加或删除。例如，聚类算法可能会从一个大图中筛选出一小组彼此不连通的顶点；而图挖掘算法则可能只输出从图中提取出的聚合统计信息。

我们选择 **纯消息传递模型** ，而不采用远程读取或其他模拟共享内存的方式，原因有两个。首先，消息传递的表达能力已经足够强大，因此没有必要引入远程读取。我们也没有发现任何图算法是消息传递无法支持的。其次，这样做在性能上更有优势。在集群环境中，从远程机器读取一个值会带来很高的延迟，而且这种延迟很难被有效隐藏。我们的消息传递模型支持异步批量发送消息，因此可以通过批量传输来摊销延迟开销。

图算法也可以写成一系列串联的 MapReduce 调用。但出于可用性和性能方面的考虑，我们选择了不同的模型。Pregel 会把顶点和边保留在执行计算的机器上，只在需要传递消息时才使用网络通信。而 MapReduce 本质上是函数式的，因此如果把图算法写成一连串的 MapReduce，就需要在每个阶段之间传递整个图的状态，通常会产生更多的通信开销和序列化开销。此外，串联 MapReduce 还需要额外协调各个阶段，这会增加编程复杂度；相比之下，Pregel 通过 superstep 迭代避免了这种复杂性。

# 3. THE C++ API

```cpp
template <typename VertexValue, typename EdgeValue, typename MessageValue>
class Vertex 
{
public:
  virtual void Compute(MessageIterator* msgs) = 0;

  const string& vertex_id() const;
  int64 superstep() const;

  const VertexValue& GetValue();
  VertexValue* MutableValue();

  OutEdgeIterator GetOutEdgeIterator();

  void SendMessageTo(const string& dest_vertex, const MessageValue& message);

  void VoteToHalt();
};
```

编写 Pregel 程序需要继承预定义的 `Vertex` 类（见图 3）。其模板参数定义了三种值类型，分别对应顶点、边和消息。每个顶点都关联一个指定类型的值。这样的统一性看起来可能有些限制，但用户可以通过使用协议缓冲区 [42] 这类灵活的类型来缓解这一问题。边类型和消息类型的行为与此类似。

需要重写虚函数 `Compute()`，该方法会在每个 superstep 中对所有活动顶点执行。

预定义的 `Vertex` 方法允许 `Compute()` 查询当前顶点及其边的信息，并向其他顶点发送消息。`Compute()` 可以通过 `GetValue()` 查看与该顶点关联的值，也可以通过 `MutableValue()` 对其进行修改。它还可以使用出边迭代器提供的方法查看和修改出边的值。这些状态更新会立即生效。由于这种可见性仅限于被修改的顶点，因此不同顶点之间并发访问值时不会产生数据竞争。

与顶点及其边关联的值，是跨 superstep 持久化的唯一每顶点状态。将框架管理的图状态限制为每个顶点或边只保留一个值，可以简化主计算循环、图的分布式存储以及故障恢复。

## 3.1 消息传递

顶点通过发送消息直接通信。每条消息都由一个消息值和目标顶点名称组成。消息值的类型由用户作为 `Vertex` 类的模板参数指定。

一个顶点在一个 superstep 中可以发送任意数量的消息。

在 superstep S 中发送给顶点 V 的所有消息，会在 V 的 `Compute()` 方法于 superstep S+1 中被调用时，通过迭代器提供给它。迭代器中的消息顺序不作保证，但可以保证消息一定会被传递，且不会重复。

一种常见的用法是，顶点 V 遍历其所有出边，并向每条边的目标顶点发送一条消息，如图 4 中的 PageRank 算法所示（见下文第 5.1 节）。不过，`dest_vertex` 不一定必须是 V 的邻居。顶点可以通过先前收到的消息得知某个非邻居的标识符，或者顶点标识符本身就是隐式已知的。例如，图可以是一个 clique，顶点标识符为众所周知的 V1 到 Vn，在这种情况下，甚至可能不需要在图中显式保存边。

当某条消息的目标顶点不存在时，我们会执行用户定义的处理程序。例如，处理程序可以创建缺失的顶点，或者从源顶点中删除那条悬空边。

## 3.2 组合器

发送消息，尤其是发送到另一台机器上的顶点，会带来一定开销。在某些情况下，用户可以帮助降低这种开销。例如，假设 `Compute()` 接收整数消息，而程序只关心它们的总和，而不是每条消息的具体值。那么系统就可以把发送给顶点 V 的多条消息合并成一条包含它们和的消息，从而减少需要传输和缓冲的消息数量。

组合器默认不启用，因为系统没有一种机械化的方法，能够自动找出一个既有用又与用户 `Compute()` 方法语义一致的组合函数。要启用这种优化，用户需要继承 `Combiner` 类，并重写虚函数 `Combine()`。

系统不会保证哪些消息会被合并（如果有的话）、传给组合器的分组方式，或者合并顺序，因此组合器只适用于交换律和结合律都成立的操作。

**对于某些算法，例如单源最短路径（第 5.2 节），我们观察到，使用组合器后消息流量减少了四倍以上。**

## 3.3 聚合器

Pregel 的聚合器是一种用于全局通信、监控和数据统计的机制。每个顶点都可以在 superstep S 中向聚合器提供一个值，系统再使用归约操作将这些值合并，得到的结果会在 superstep S+1 中对所有顶点可见。Pregel 提供了若干预定义聚合器，例如作用于各种整数或字符串类型的 min、max 或 sum 操作。

聚合器可以用于统计。例如，对每个顶点的出度应用 sum 聚合器，就可以得到图中边的总数。更复杂的归约操作还可以生成某个统计量的直方图。

**聚合器也可以用于全局协调。例如，可以先执行 `Compute()` 中的一个分支，直到某个 and 聚合器判断所有顶点都满足某个条件，然后再切换到另一个分支继续执行直到结束。将 min 或 max 聚合器应用于顶点 ID，还可以选出一个顶点在算法中承担特殊角色。**

**要定义新的聚合器，用户需要继承预定义的 `Aggregator` 类，并指定如何由第一个输入值初始化聚合结果，以及如何把多个部分聚合结果归约成一个结果。聚合操作应当满足交换律和结合律。**

**默认情况下，聚合器只会归约单个 superstep 中的输入值，但也可以定义一种 sticky 聚合器，使用所有 superstep 的输入值。这对于维护一个全局边数很有用，例如只在边被添加或删除时才调整该计数。**

**也可以有更高级的用途。例如，聚合器可以用于实现 ∆-stepping 最短路径算法 [37] 中的分布式优先队列。每个顶点会根据其暂定距离被分配到一个优先级桶。在一个 superstep 中，顶点把自己的索引贡献给 `min` 聚合器。最小值会在下一个 superstep 中广播给所有 worker，最低索引桶中的顶点随后松弛边。**

### 3.4 拓扑变更

一些图算法需要改变图的拓扑结构。例如，聚类算法可能会用一个顶点替换每个簇，而最小生成树算法可能会删除除树边之外的所有边。就像用户的 `Compute()` 函数可以发送消息一样，它也可以发出添加或删除顶点或边的请求。

多个顶点可能在同一个 superstep 中发出冲突请求，例如两个请求同时添加顶点 V，但初始值不同。我们使用两种机制来保证确定性：部分顺序和处理程序。

和消息一样，拓扑变更会在发出请求的那个 superstep 之后的下一个 superstep 中生效。在那个 superstep 内，先执行删除操作，其中边删除先于顶点删除，因为删除顶点会隐式删除其所有出边。随后执行添加操作，其中顶点添加先于边添加；并且所有变更都会先于对 `Compute()` 的调用。这样的部分顺序能够为大多数冲突产生确定性的结果。

剩余的冲突则由用户定义的处理程序解决。如果在同一个 superstep 中有多个创建同一顶点的请求，默认情况下系统会任意选择其中一个；但有特殊需求的用户可以通过在其 `Vertex` 子类中定义合适的处理方法，指定更好的冲突解决策略。相同的处理机制也用于解决多个顶点删除请求，或者多个边添加/删除请求引起的冲突。我们把冲突处理交给 handler，是为了让 `Compute()` 的代码保持简单。这会限制 handler 与 `Compute()` 之间的交互，但在实践中并没有成为问题。

我们的协调机制是惰性的：全局变更直到真正应用时才需要协调。这样的设计有利于流式处理。其直觉是，凡是涉及修改顶点 V 的冲突，都由 V 自己来处理。

Pregel 还支持纯局部变更，也就是某个顶点添加或删除自己的出边，或者删除自己。局部变更不会引入冲突，而且可以立即生效；这通过采用更简单的顺序编程语义，简化了分布式编程。

### 3.5 输入与输出

图可以有多种文件格式，例如文本文件、关系数据库中的一组顶点，或者 Bigtable [9] 中的行。为了避免强行限定某一种文件格式，Pregel 将“把输入文件解释成图”的任务与“进行图计算”的任务解耦。类似地，输出也可以以任意格式生成，并以最适合具体应用的形式存储。Pregel 库为许多常见文件格式提供了读写器，但对于有特殊需求的用户，也可以通过继承抽象基类 `Reader` 和 `Writer` 自己实现。

# 4. IMPLEMENTATION

Our applications typically execute on a cluster management system that schedules jobs to optimize resource allocation, sometimes killing instances or moving them to different machines. The system includes a name service, so that instances can be referred to by logical names independent of their current binding to a physical machine. Persistent data is stored as files on a distributed storage system, GFS [19], or in Bigtable [9], and temporary data such as buffered messages on local disk.
我们的应用程序通常在集群管理系统上执行，该系统调度作业以优化资源分配，有时会终止实例或将其移动到不同的机器。该系统包括一个名称服务，以便可以独立于当前绑定到的物理机器来引用逻辑名称的实例。持久数据存储为分布式存储系统GFS [19]或Bigtable[9]上的文件，临时数据（如缓冲的消息）存储在本地磁盘上。

4.1 Basic architecture
4.1 基本架构

The Pregel library divides a graph into partitions, each consisting of a set of vertices and all of those vertices' outgoing edges. Assignment of a vertex to a partition depends solely on the vertex ID, which implies it is possible to know which partition a given vertex belongs to even if the vertex is owned by a different machine, or even if the vertex does not yet exist. The default partitioning function is just hash(ID) mod N, where N is the number of partitions, but users can replace it.
Pregel库将图划分为分区，每个分区由一组顶点和这些顶点的所有出边组成。顶点到分区的分配仅取决于顶点ID，这意味着即使顶点由不同的机器拥有，甚至顶点尚不存在，也可能知道给定顶点属于哪个分区。默认分区函数只是hash(ID) mod N，其中N是分区数，但用户可以替换它。

The assignment of vertices to worker machines is the main place where distribution is not transparent in Pregel. Some applications work well with the default assignment, but some benefit from defining custom assignment functions to better exploit locality inherent in the graph. For example, a typical heuristic employed for the Web graph is to colocate vertices representing pages of the same site.
顶点到worker机器的分配是Pregel中分布不透明的主要地方。一些应用程序使用默认分配效果很好，但一些可以通过定义自定义分配函数更好地利用图中固有的局部性而受益。例如，用于Web图的一个典型启发式方法是将表示同一站点页面的顶点放在一起。

In the absence of faults, the execution of a Pregel program consists of several stages:
在没有故障的情况下，Pregel程序的执行由几个阶段组成：

1. Many copies of the user program begin executing on a cluster of machines. One of these copies acts as the master. It is not assigned any portion of the graph, but is responsible for coordinating worker activity. The workers use the cluster management system's name service to discover the master's location, and send registration messages to the master.
1. 用户程序的许多副本开始在机器集群上执行。其中一个副本充当master。它没有被分配图的任何部分，但负责协调worker活动。worker使用集群管理系统的名称服务来发现master的位置，并向master发送注册消息。

2. The master determines how many partitions the graph will have, and assigns one or more partitions to each worker machine. The number may be controlled by the user. Having more than one partition per worker allows parallelism among the partitions and better load balancing, and will usually improve performance. Each worker is responsible for maintaining the state of its section of the graph, executing the user's Compute() method on its vertices, and managing messages to and from other workers. Each worker is given the complete set of assignments for all workers.
2. master确定图将有多少个分区，并为每个worker机器分配一个或多个分区。这个数字可以由用户控制。每个worker有多个分区允许分区之间的并行性和更好的负载平衡，通常会提高性能。每个worker负责维护其部分图的状态，在其顶点上执行用户的Compute()方法，以及管理与来自其他worker的消息。每个worker都被给予所有worker的完整分配集。

3. The master assigns a portion of the user's input to each worker. The input is treated as a set of records, each of which contains an arbitrary number of vertices and edges. The division of inputs is orthogonal to the partitioning of the graph itself, and is typically based on file boundaries. If a worker loads a vertex that belongs to that worker's section of the graph, the appropriate data structures (Section 4.3) are immediately updated. Otherwise the worker enqueues a message to the remote peer that owns the vertex. After the input has finished loading, all vertices are marked as active.
3. master将用户输入的一部分分配给每个worker。输入被视为一组记录，每条记录包含任意数量的顶点和边。输入的划分与图本身的分区正交，通常基于文件边界。如果worker加载属于该worker图部分的顶点，则立即更新适当的数据结构（第4.3节）。否则，worker将消息加入队列，发送给拥有该顶点的远程对等点。输入加载完成后，所有顶点都被标记为活动状态。

4. The master instructs each worker to perform a superstep. The worker loops through its active vertices, using one thread for each partition. The worker calls Compute() for each active vertex, delivering messages that were sent in the previous superstep. Messages are sent asynchronously, to enable overlapping of computation and communication and batching, but are delivered before the end of the superstep. When the worker is finished it responds to the master, telling the master how many vertices will be active in the next superstep. This step is repeated as long as any vertices are active, or any messages are in transit.
4. master指示每个worker执行一个superstep。worker循环遍历其活动顶点，每个分区使用一个线程。worker为每个活动顶点调用Compute()，传递在前一个superstep中发送的消息。消息异步发送，以实现计算和通信的重叠及批处理，但在superstep结束前交付。当worker完成时，它响应master，告知master下一个superstep中将有多少个顶点处于活动状态。只要有任何顶点处于活动状态或有任何消息在传输中，就重复此步骤。

5. After the computation halts, the master may instruct each worker to save its portion of the graph.
5. 计算停止后，master可以指示每个worker保存其部分的图。

4.2 Fault tolerance
4.2 容错

Fault tolerance is achieved through checkpointing. At the beginning of a superstep, the master instructs the workers to save the state of their partitions to persistent storage, including vertex values, edge values, and incoming messages; the master separately saves the aggregator values.
容错通过检查点实现。在superstep开始时，master指示worker将其分区的状态保存到持久存储，包括顶点值、边值和传入消息；master单独保存聚合器值。

Worker failures are detected using regular "ping" messages that the master issues to workers. If a worker does not receive a ping message after a specified interval, the worker process terminates. If the master does not hear back from a worker, the master marks that worker process as failed.
使用master向worker发出的常规"ping"消息检测worker故障。如果worker在指定间隔后未收到ping消息，worker进程终止。如果master未收到worker的回复，master将该worker进程标记为失败。

When one or more workers fail, the current state of the partitions assigned to these workers is lost. The master reassigns graph partitions to the currently available set of workers, and they all reload their partition state from the most recent available checkpoint at the beginning of a superstep S. That checkpoint may be several supersteps earlier than the latest superstep S0 completed by any partition before the failure, requiring that recovery repeat the missing supersteps. We select checkpoint frequency based on a mean time to failure model [13], balancing checkpoint cost against expected recovery cost.
当一个或多个worker失败时，分配给这些worker的分区当前状态会丢失。master将图分区重新分配给当前可用的worker集，它们都在superstep S开始时从最近的可用检查点重新加载其分区状态。该检查点可能比任何分区在故障前完成的最新superstep S0早几个superstep，需要恢复重复缺失的supersteps。我们基于平均故障时间模型[13]选择检查点频率，平衡检查点成本与预期恢复成本。

Confined recovery is under development to improve the cost and latency of recovery. In addition to the basic checkpoints, the workers also log outgoing messages from their assigned partitions during graph loading and supersteps. Recovery is then confined to the lost partitions, which are recovered from checkpoints. The system recomputes the missing supersteps up to S0 using logged messages from healthy partitions and recalculated ones from recovering partitions. This approach saves compute resources during recovery by only recomputing lost partitions, and can improve the latency of recovery since each worker may be recovering fewer partitions. Saving the outgoing messages adds overhead, but a typical machine has adequate disk bandwidth to ensure that I/O does not become the bottleneck.
受限恢复正在开发中以改善恢复的成本和延迟。除了基本检查点外，worker还在图加载和supersteps期间记录来自其分配分区的传出消息。然后恢复仅限于丢失的分区，从检查点恢复。系统使用来自健康分区的已记录消息和来自恢复分区的重新计算的消息重新计算缺失的supersteps直到S0。这种方法通过仅重新计算丢失的分区来节省恢复期间的计算资源，并且可以改善恢复延迟，因为每个worker可能恢复较少的分区。保存传出消息会增加开销，但典型机器有足够的磁盘带宽来确保I/O不会成为瓶颈。

Confined recovery requires the user algorithm to be deterministic, to avoid inconsistencies due to mixing saved messages from the original execution with new messages from the recovery. Randomized algorithms can be made deterministic by seeding a pseudorandom number generator deterministically based on the superstep and the partition. Nondeterministic algorithms can disable confined recovery and fall back to the basic recovery mechanism.
受限恢复要求用户算法是确定性的，以避免由于将原始执行的保存消息与恢复的新消息混合而导致的不一致。随机算法可以通过基于superstep和分区确定性地植入伪随机数生成器来使其具有确定性。非确定性算法可以禁用受限恢复并回退到基本恢复机制。

4.3 Worker implementation
4.3 Worker实现

A worker machine maintains the state of its portion of the graph in memory. Conceptually this can be thought of as a map from vertex ID to the state of each vertex, where the state of each vertex consists of its current value, a list of its outgoing edges (the vertex ID for the edge's target, and the edge's current value), a queue containing incoming messages, and a flag specifying whether the vertex is active.
worker机器在内存中维护其部分图的状态。概念上，这可以被认为是顶点ID到每个顶点状态的映射，其中每个顶点的状态包括其当前值、出边列表（边的目标的顶点ID和边的当前值）、包含传入消息的队列以及指定顶点是否处于活动状态的标志。

When the worker performs a superstep it loops through all vertices and calls Compute(), passing it the current value, an iterator to the incoming messages, and an iterator to the outgoing edges. There is no access to incoming edges because each incoming edge is part of a list owned by the source vertex, in general on a different machine.
当worker执行superstep时，它循环遍历所有顶点并调用Compute()，向其传递当前值、传入消息的迭代器和出边的迭代器。没有对入边的访问，因为每条入边都是源顶点拥有的列表的一部分，通常在不同的机器上。

For performance reasons, the active vertex flags are stored separately from the incoming message queues. Furthermore, while only a single copy of the vertex and edge values exists, two copies of the active vertex flags and the incoming message queue exist: one for the current superstep and one for the next superstep. While a worker processes its vertices in superstep S it is simultaneously, in another thread, receiving messages from other workers executing the same superstep. Since vertices receive messages that were sent in the previous superstep (see Section 2), messages for supersteps S and S + 1 must be kept separate. Similarly, arrival of a message for a vertex V means that V will be active in the next superstep, not necessarily the current one.
出于性能原因，活动顶点标志与传入消息队列分开存储。此外，虽然顶点 和边值只存在一个副本，但活动顶点标志和传入消息队列存在两个副本：一个用于当前superstep，一个用于下一个superstep。当worker在其superstep S中处理顶点时，它同时在另一个线程中接收来自执行相同superstep的其他worker的消息。由于顶点接收在前一个superstep中发送的消息（见第2节），必须将superstep S和S+1的消息保持分开。类似地，顶点V的消息到达意味着V将在下一个superstep中处于活动状态，不一定是当前superstep。

When Compute() requests sending a message to another vertex, the worker process first determines whether the destination vertex is owned by a remote worker machine, or by the same worker that owns the sender. In the remote case the message is buffered for delivery to the destination worker. When the buffer sizes reach a threshold, the largest buffers are asynchronously flushed, delivering each to its destination worker as a single network message. In the local case an optimization is possible: the message is placed directly in the destination vertex's incoming message queue.
当Compute()请求向另一个顶点发送消息时，worker进程首先确定目标顶点是由远程worker机器还是由拥有发送者的同一worker拥有。在远程情况下，消息被缓冲以交付给目标worker。当缓冲区大小达到阈值时，最大的缓冲区被异步刷新，作为单个网络消息交付给其目标worker。在本地情况下，可以进行优化：消息直接放在目标顶点的传入消息队列中。

If the user has provided a Combiner (Section 3.2), it is applied when messages are added to the outgoing message queue and when they are received at the incoming message queue. The latter does not reduce network usage, but does reduce the space needed to store messages.
如果用户提供了Combiner（第3.2节），它会在消息添加到传出消息队列时以及在传入消息队列接收时应用。后者不会减少网络使用，但确实减少了存储消息所需的空间。

4.4 Master implementation
4.4 Master实现

The master is primarily responsible for coordinating the activities of workers. Each worker is assigned a unique identifier at the time of its registration. The master maintains a list of all workers currently known to be alive, including the worker's unique identifier, its addressing information, and which portion of the graph it has been assigned. The size of the master's data structures is proportional to the number of partitions, not the number of vertices or edges, so a single master can coordinate computation for even a very large graph.
master主要负责协调worker的活动。每个worker在注册时被分配一个唯一标识符。master维护当前已知存活的所有worker的列表，包括worker的唯一标识符、其寻址信息以及分配给它的图的哪一部分。master数据结构的大小与分区数成正比，而不是与顶点数或边数成正比，因此单个master可以协调即使是非常大的图的计算。

Most master operations, including input, output, computation, and saving and resuming from checkpoints, are terminated at barriers: the master sends the same request to every worker that was known to be alive at the time the operation begins, and waits for a response from every worker. If any worker fails, the master enters recovery mode as described in section 4.2. If the barrier synchronization succeeds, the master proceeds to the next stage. In the case of a computation barrier, for example, the master increments the global superstep index and proceeds to the next superstep.
大多数master操作，包括输入、输出、计算以及保存和从检查点恢复，都在屏障处终止：master向操作开始时已知存活的所有worker发送相同的请求，并等待每个worker的响应。如果任何worker失败，master进入第4.2节中描述的恢复模式。如果屏障同步成功，master进入下一阶段。例如，在计算屏障的情况下，master增加全局superstep索引并进入下一个superstep。

The master also maintains statistics about the progress of computation and the state of the graph, such as the total size of the graph, a histogram of its distribution of out-degrees, the number of active vertices, the timing and message traffic of recent supersteps, and the values of all user-defined aggregators. To enable user monitoring, the master runs an HTTP server that displays this information.
master还维护关于计算进度和图状态的统计信息，例如图的 total size、出度分布的直方图、活动顶点数、最近supersteps的时序和消息流量，以及所有用户定义聚合器的值。为了启用用户监控，master运行一个HTTP服务器来显示此信息。

4.5 Aggregators
4.5 聚合器

An aggregator (Section 3.3) computes a single global value by applying an aggregation function to a set of values that the user supplies. Each worker maintains a collection of aggregator instances, identified by a type name and instance name. When a worker executes a superstep for any partition of the graph, the worker combines all of the values supplied to an aggregator instance into a single local value: an aggregator that is partially reduced over all of the worker's vertices in the partition. At the end of the superstep workers form a tree to reduce partially reduced aggregators into global values and deliver them to the master. We use a tree-based reduction—rather than pipelining with a chain of workers—to parallelize the use of CPU during reduction. The master sends the global values to all workers at the beginning of the next superstep.
聚合器（第3.3节）通过将聚合函数应用于用户提供的一组值来计算单个全局值。每个worker维护一组聚合器实例，由类型名称和实例名称标识。当worker对图的任何分区执行superstep时，worker将提供给聚合器实例的所有值组合成单个本地值：一个在分区中所有worker顶点上部分归约的聚合器。在superstep结束时，worker形成一个树以将部分归约的聚合器归约为全局值并将它们交付给master。我们使用基于树的归约——而不是与worker链的流水线——来在归约期间并行化CPU的使用。master在下一个superstep开始时将全局值发送给所有worker。

# 5. APPLICATIONS
# 5. 应用

This section presents four examples that are simplified versions of algorithms developed by Pregel users to solve real problems: Page Rank, Shortest Paths, Bipartite Matching, and a Semi-Clustering algorithm.
本节展示了四个简化版本的示例，这些示例由Pregel用户开发以解决实际问题：Page Rank、最短路径、二部匹配和半聚类算法。

5.1 PageRank
5.1 PageRank

A Pregel implementation of a PageRank algorithm [7] is shown in Figure 4. The PageRankVertex class inherits from Vertex. Its vertex value type is double to store a tentative PageRank, and its message type is double to carry PageRank fractions, while the edge value type is void because edges do not store information. We assume that the graph is initialized so that in superstep 0, the value of each vertex is 1 / NumVertices(). In each of the first 30 supersteps, each vertex sends along each outgoing edge its tentative PageRank divided by the number of outgoing edges. Starting from superstep 1, each vertex sums up the values arriving on messages into sum and sets its own tentative PageRank to 0.15/NumVertices() + 0.85 × sum. After reaching superstep 30, no further messages are sent and each vertex votes to halt. In practice, a PageRank algorithm would run until convergence was achieved, and aggregators would be useful for detecting the convergence condition.
图4显示了PageRank算法[7]的Pregel实现。PageRankVertex类继承自Vertex。其顶点值类型为double以存储暂定PageRank，消息类型为double以承载PageRank分数，而边值类型为void，因为边不存储信息。我们假设图被初始化为在superstep 0中，每个顶点的值为1/NumVertices()。在前30个supersteps中的每一个中，每个顶点沿每个出边发送其暂定PageRank除以出边数。从superstep 1开始，每个顶点将消息中的值求和到sum中，并将其自己的暂定PageRank设置为0.15/NumVertices() + 0.85×sum。达到superstep 30后，不再发送消息，每个顶点投票停止。在实践中，PageRank算法会运行直到达到收敛，聚合器可用于检测收敛条件。

5.2 Shortest Paths
5.2 最短路径

Shortest paths problems are among the best known problems in graph theory and arise in a wide variety of applications [10, 24], with several important variants. The single-source shortest paths problem requires finding a shortest path between a single source vertex and every other vertex in the graph. The s-t shortest path problem requires finding a single shortest path between given vertices s and t; it has obvious practical applications like driving directions and has received a great deal of attention. It is also relatively easy—solutions in typical graphs like road networks visit a tiny fraction of vertices, with Lumsdaine et al [31] observing visits to 80,000 vertices out of 32 million in one example. A third variant, all-pairs shortest paths, is impractical for large graphs because of its O(|V|2) storage requirements.
最短路径问题是图论中最著名的问题之一，出现在各种应用中[10, 24]，有几种重要的变体。单源最短路径问题要求找到单个源顶点与图中每个其他顶点之间的最短路径。s-t最短路径问题要求找到给定顶点s和t之间的单个最短路径；它有明显的实际应用如驾驶导航，并受到广泛关注。它也相对容易——在诸如道路网络的典型图中，解决方案访问很少一部分顶点，Lumsdaine等[31]观察到在一个例子中访问了3200万个顶点中的80000个。第三种变体，全对最短路径，由于其O(|V|2)的存储需求，对大型图是不切实际的。

For simplicity and conciseness, we focus here on the single-source variant that fits Pregel's target of large-scale graphs very well, but offers more interesting scaling data than the s-t shortest path problem. An implementation is shown in Figure 5.
为简单和简洁起，我们这里专注于单源变体，它非常适合Pregel的大规模图目标，但比s-t最短路径问题提供更有趣的扩展数据。图5显示了一个实现。

In this algorithm, we assume the value associated with each vertex is initialized to INF (a constant larger than any feasible distance in the graph from the source vertex). In each superstep, each vertex first receives, as messages from its neighbors, updated potential minimum distances from the source vertex. If the minimum of these updates is less than the value currently associated with the vertex, then this vertex updates its value and sends out potential updates to its neighbors, consisting of the weight of each outgoing edge added to the newly found minimum distance. In the first superstep, only the source vertex will update its value (from INF to zero) and send updates to its immediate neighbors. These neighbors in turn will update their values and send messages, resulting in a wavefront of updates through the graph. The algorithm terminates when no more updates occur, after which the value associated with each vertex denotes the minimum distance from the source vertex to that vertex. (The value INF denotes that the vertex cannot be reached at all.) Termination is guaranteed if all edge weights are non-negative.
在这个算法中，我们假设与每个顶点关联的值被初始化为INF（一个大于从源顶点到图中任何可行距离的常数）。在每个superstep中，每个顶点首先作为来自其邻居的消息接收从源顶点更新的潜在最小距离。如果这些更新的最小值小于当前与顶点关联的值，则该顶点更新其值并向邻居发送潜在更新，包括每条出边的权重加上新发现的最小距离。在第一个superstep中，只有源顶点会更新其值（从INF到0）并向其直接邻居发送更新。这些邻居反过来会更新它们的值并发送消息，导致更新通过图的波前传播。当不再有更新发生时算法终止，之后与每个顶点关联的值表示从源顶点到该顶点的最小距离。（值INF表示该顶点根本无法到达。）如果所有边权重都是非负的，则保证终止。

Messages in this algorithm consist of potential shorter distances. Since the receiving vertex is ultimately only interested in the minimum, this algorithm is amenable to optimization using a combiner (Section 3.2). The combiner shown in Figure 6 greatly reduces the amount of data sent between workers, as well as the amount of data buffered prior to executing the next superstep. While the code in Figure 5 only computes distances, modifying it to compute the shortest paths tree as well is quite straightforward.
这个算法中的消息由潜在更短距离组成。由于接收顶点最终只对最小值感兴趣，这个算法适合使用combiner进行优化（第3.2节）。图6所示的combiner大大减少了worker之间发送的数据量，以及在执行下一个superstep之前缓冲的数据量。虽然图5中的代码只计算距离，但修改它以同时计算最短路径树也非常简单。

This algorithm may perform many more comparisons than sequential counterparts such as Dijkstra or Bellman-Ford [5, 15, 17, 24], but it is able to solve the shortest paths problem at a scale that is infeasible with any single-machine implementation. More advanced parallel algorithms exist, e.g., Thorup [44] or the ∆-stepping method [37], and have been used as the basis for special-purpose parallel shortest paths implementations [12, 32]. Such advanced algorithms can also be expressed in the Pregel framework. The simplicity of the implementation in Figure 5, however, together with the already acceptable performance (see Section 6), may appeal to users who can't do extensive tuning or customization.
这个算法可能比顺序对应算法如Dijkstra或Bellman-Ford[5, 15, 17, 24]执行更多的比较，但它能够以任何单机器实现都不可行的规模解决最短路径问题。存在更先进的并行算法，例如Thorup[44]或∆-stepping方法[37]，并已被用作专用并行最短路径实现[12, 32]的基础。这样的先进算法也可以在Pregel框架中表达。然而，图5中实现的简单性以及已经可接受的性能（见第6节）可能吸引无法进行大量调优或定制的用户。

5.3 Bipartite Matching
5.3 二部匹配

The input to a bipartite matching algorithm consists of two distinct sets of vertices with edges only between the sets, and the output is a subset of edges with no common endpoints. A maximal matching is one to which no additional edge can be added without sharing an endpoint. We implemented a randomized maximal matching algorithm [1] and a maximum-weight bipartite matching algorithm [4]; we describe the former here.
二部匹配算法的输入由两个不同的顶点集组成，边只存在于集合之间，输出是一个没有公共端点的边的子集。最大匹配是一个不能在不共享端点的情况下添加任何额外边的匹配。我们实现了一个随机最大匹配算法[1]和一个最大权二部匹配算法[4]；我们在这里描述前者。

In the Pregel implementation of this algorithm the vertex value is a tuple of two values: a flag indicating which set the vertex is in (L or R), and the name of its matched vertex once known. The edge value has type void (edges carry no information), and the messages are boolean. The algorithm proceeds in cycles of four phases, where the phase index is just the superstep index mod 4, using a three-way handshake.
在这个算法的Pregel实现中，顶点值是一个二元组：一个标志指示顶点属于哪个集合（L或R），以及一旦知道就记录其匹配顶点的名称。边值类型为void（边不携带信息），消息是布尔值。算法以四阶段的周期进行，其中阶段索引只是superstep索引mod 4，使用三方握手。

In phase 0 of a cycle, each left vertex not yet matched sends a message to each of its neighbors to request a match, and then unconditionally votes to halt. If it sent no messages (because it is already matched, or has no outgoing edges), or if all the message recipients are already matched, it will never be reactivated. Otherwise, it will receive a response in two supersteps and reactivate.
在周期的第0阶段，每个尚未匹配的左侧顶点向其每个邻居发送消息请求匹配，然后无条件投票停止。如果它没有发送消息（因为它已经匹配或没有出边），或者所有消息接收者都已匹配，它将永远不会被重新激活。否则，它将在两个supersteps中收到响应并重新激活。

In phase 1 of a cycle, each right vertex not yet matched randomly chooses one of the messages it receives, sends a message granting that request, and sends messages to other requestors denying it. Then it unconditionally votes to halt.
在周期的第1阶段，每个尚未匹配的右侧顶点随机选择它接收到的消息之一，发送一条消息授予该请求，并向其他请求者发送拒绝消息。然后它无条件投票停止。

In phase 2 of a cycle, each left vertex not yet matched chooses one of the grants it receives and sends an acceptance message. Left vertices that are already matched will never execute this phase, since they will not have sent a message in phase 0.
在周期的第2阶段，每个尚未匹配的左侧顶点选择它接收到的授权之一并发送接受消息。已匹配的左侧顶点永远不会执行此阶段，因为它们在第0阶段不会发送消息。

Finally, in phase 3, an unmatched right vertex receives at most one acceptance message. It notes the matched node and unconditionally votes to halt—it has nothing further to do.
最后，在第3阶段，未匹配的右侧顶点最多接收一条接受消息。它记下匹配的节点并无条件投票停止——它没有更多事情可做了。

5.4 Semi-Clustering
5.4 半聚类

Pregel has been used for several different versions of clustering. One version, semi-clustering, arises in social graphs. Vertices in a social graph typically represent people, and edges represent connections between them. Edges may be based on explicit actions (e.g., adding a friend in a social networking site), or may be inferred from people's behavior (e.g., email conversations or co-publication). Edges may have weights, to represent the interactions' frequency or strength.
Pregel已被用于几个不同版本的聚类。其中一个版本，半聚类，产生于社交图。社交图中的顶点通常代表人们，边代表他们之间的连接。边可以基于明确的行为（例如在社交网站上添加朋友），或者可以从人们的行为中推断（例如电子邮件对话或合著）。边可以有权重，以表示交互的频率或强度。

A semi-cluster in a social graph is a group of people who interact frequently with each other and less frequently with others. What distinguishes it from ordinary clustering is that a vertex may belong to more than one semi-cluster. This section describes a parallel greedy semi-clustering algorithm. Its input is a weighted, undirected graph (represented in Pregel by constructing each edge twice, once in each direction) and its output is at most Cmax semi-clusters, each containing at most Vmax vertices, where Cmax and Vmax are user-specified parameters.
社交图中的半聚类是一组彼此频繁交互而与他人较少交互的人。它与普通聚类的区别在于，一个顶点可以属于多个半聚类。本节描述了一种并行贪婪半聚类算法。它的输入是一个加权的无向图（在Pregel中通过将每条边构造两次来表示，每个方向一次），其输出最多是Cmax个半聚类，每个最多包含Vmax个顶点，其中Cmax和Vmax是用户指定的参数。

A semi-cluster c is assigned a score, Sc = (Ic − fB×Bc) / (Vc(Vc − 1)/2), where Ic is the sum of the weights of all internal edges, Bc is the sum of the weights of all boundary edges (i.e., edges connecting a vertex in the semi-cluster to one outside it), Vc is the number of vertices in the semi-cluster, and fB, the boundary edge score factor, is a user-specified parameter, usually between 0 and 1. The score is normalized, i.e., divided by the number of edges in a clique of size Vc, so that large clusters do not receive artificially high scores.
半聚类c被分配一个分数，Sc = (Ic − fB×Bc) / (Vc(Vc − 1)/2)，其中Ic是所有内部边的权重之和，Bc是所有边界边的权重之和（即连接半聚类中顶点与外部顶点的边），Vc是半聚类中的顶点数，fB是边界边分数因子，是一个用户指定的参数，通常在0和1之间。分数被规范化，即除以大小为Vc的clique中的边数，这样大聚类不会获得人为的高分。

Each vertex V maintains a list containing at most Cmax semi-clusters, sorted by score. In superstep 0 V enters itself in that list as a semi-cluster of size 1 and score 1, and publishes itself to all of its neighbors. In subsequent supersteps:
每个顶点V维护一个最多包含Cmax个半聚类的列表，按分数排序。在superstep 0中，V在该列表中输入自己作为一个大小为1分数为1的半聚类，并向其所有邻居发布自己。在后续supersteps中：

• Vertex V iterates over the semi-clusters c1,...,ck sent to it on the previous superstep. If a semi-cluster c does not already contain V, and Vc < Vmax, then V is added to c to form c'.
• 顶点V遍历在上一个superstep发送给它的半聚类c1,...,ck。如果半聚类c不包含V，且Vc < Vmax，则将V添加到c中形成c'。

• The semi-clusters c1, ..., ck, c'1, ..., c'k are sorted by their scores, and the best ones are sent to V's neighbors.
• 半聚类c1, ..., ck, c'1, ..., c'k按其分数排序，最好的被发送到V的邻居。

• Vertex V updates its list of semi-clusters with the semi-clusters from c1, ..., ck, c'1, ..., c'k that contain V.
• 顶点V用c1, ..., ck, c'1, ..., c'k中包含V的半聚类更新其半聚类列表。

The algorithm terminates either when the semi-clusters stop changing or (to improve performance) when the number of supersteps reaches a user-specified limit. At that point the list of best semi-cluster candidates for each vertex may be aggregated into a global list of best semi-clusters.
算法在半聚类停止变化时或者（在提高性能方面）当superstep数量达到用户指定的限制时终止。在那一点上，每个顶点的最佳半聚类候选列表可以聚合为全局最佳半聚类列表。

6. EXPERIMENTS
6. 实验

We conducted various experiments with the single-source shortest paths (SSSP) implementation of Section 5.2 on a cluster of 300 multicore commodity PCs. We report runtimes for binary trees (to study scaling properties) and log-normal random graphs (to study the performance in a more realistic setting) using various graph sizes with the weights of all edges implicitly set to 1.
我们使用第5.2节的单源最短路径（SSSP）实现在300台多核商品PC的集群上进行了各种实验。我们报告了二叉树（研究缩放特性）和对数正态随机图（在更现实的环境中研究性能）的运行时间，使用各种图大小，所有边的权重隐式设置为1。

The time for initializing the cluster, generating the test graphs in-memory, and verifying results is not included in the measurements. Since all experiments could run in a relatively short time, failure probability was low, and checkpointing was disabled.
初始化集群、在内存中生成测试图和验证结果的时间不包括在测量中。由于所有实验都能在相对较短的时间内运行，故障概率很低， checkpointing被禁用。

As an indication of how Pregel scales with worker tasks, Figure 7 shows shortest paths runtimes for a binary tree with a billion vertices (and, thus, a billion minus one edges) when the number of Pregel workers varies from 50 to 800. The drop from 174 to 17.3 seconds using 16 times as many workers represents a speedup of about 10.
作为Pregel如何随worker任务缩放的指示，图7显示了当Pregel worker数量从50变化到800时，具有十亿个顶点（因此是十亿减一条边）的二叉树的最短路径运行时间。使用16倍的worker从174秒下降到17.3秒代表约10倍的加速。

To show how Pregel scales with graph size, Figure 8 presents shortest paths runtimes for binary trees varying in size from a billion to 50 billion vertices, now using a fixed number of 800 worker tasks scheduled on 300 multicore machines. Here the increase from 17.3 to 702 seconds demonstrates that for graphs with a low average outdegree the runtime increases linearly in the graph size.
为了显示Pregel如何随图大小缩放，图8呈现了从十亿到500亿顶点大小变化的二叉树的最短路径运行时间，现在使用在300台多核机器上调度的800个固定worker任务。从17.3秒增加到702秒表明对于平均出度较低的图，运行时间随图大小线性增加。

Although the previous experiments give an indication of how Pregel scales in workers and graph size, binary trees are obviously not representative of graphs encountered in practice. Therefore, we also conducted experiments with random graphs that use a log-normal distribution of outdegrees, p(d) = (1/(d√(2π)σ))e^(-(ln d−μ)^2/(2σ^2)) with μ = 4 and σ = 1.3, for which the mean outdegree is 127.1. Such a distribution resembles many real-world large-scale graphs, such as the web graph or social networks, where most vertices have a relatively small degree but some outliers are much larger—a hundred thousand or more. Figure 9 shows shortest paths runtimes for such graphs varying in size from 10 million to a billion vertices (and thus over 127 billion edges), again with 800 worker tasks scheduled on 300 multicore machines. Running shortest paths for the largest graph took a little over 10 minutes.
虽然前面的实验给出了Pregel如何在worker和图大小方面缩放的指示，但二叉树显然不代表实践中遇到的图。因此，我们还使用对数正态出度分布的随机图进行了实验，p(d) = (1/(d√(2π)σ))e^(-(ln d−μ)^2/(2σ^2))，其中μ = 4和σ = 1.3，平均出度为127.1。这样的分布类似于许多现实世界的大规模图，如网页图或社交网络，其中大多数顶点具有相对较小的度，但一些异常值大得多——十万或更多。图9显示了这种图的最短路径运行时间，大小从1000万到10亿顶点（因此超过1270亿条边），同样使用在300台多核机器上调度的800个worker任务。运行最大图的最短路径花了大约10分钟。

In all experiments the graph was partitioned among workers using the default partitioning function based on a random hash; a topology-aware partitioning function would give better performance. Also, a naïve parallel shortest paths algorithm was used; here too a more advanced algorithm would perform better. Therefore, the results of the experiments in this section should not be interpreted as the best possible runtime of shortest paths using Pregel. Instead, the results are meant to show that satisfactory performance can be obtained with relatively little coding effort. In fact, our results for one billion vertices and edges are comparable to the ∆-stepping results from Parallel BGL [31] mentioned in the next section for a cluster of 112 processors on a graph of 256 million vertices and one billion edges, and Pregel scales better beyond that size.
在所有实验中，图使用基于随机哈希的默认分区函数在worker之间分区；感知拓扑的分区函数会给出更好的性能。此外，使用了简单的并行最短路径算法；这里更先进的算法也会表现得更好。因此，本节实验的结果不应被解释为使用Pregel的最短路径最佳可能运行时间。相反，结果是为了表明可以用相对较少的编码工作获得令人满意的性能。事实上，我们对十亿顶点和边的结果与下一节中提到的Parallel BGL[31]在112处理器集群上对2.56亿顶点和十亿边图的∆-stepping结果相当，并且Pregel在该规模以上扩展得更好。

7. RELATED WORK
7. 相关工作

Pregel is a distributed programming framework, focused on providing users with a natural API for programming graph algorithms while managing the details of distribution invisibly, including messaging and fault tolerance. It is similar in concept to MapReduce [14], but with a natural graph API and much more efficient support for iterative computations over the graph. This graph focus also distinguishes it from other frameworks that hide distribution details such as Sawzall [41], Pig Latin [40], and Dryad [27, 47]. Pregel is also different because it implements a stateful model where long-lived processes compute, communicate, and modify local state, rather than a dataflow model where any process computes solely on input data and produces output data input by other processes.
Pregel是一个分布式编程框架，专注于为用户提供一个自然的API来编程图算法，同时不可见地管理分布的细节，包括消息传递和容错。它在概念上类似于MapReduce[14]，但具有自然的图API和对图上迭代计算更高效的支持。这种图的重点也将其与其他隐藏分布细节的框架（如Sawzall[41]、Pig Latin[40]和Dryad[27, 47]）区分开来。Pregel也是不同的，因为它实现了一个有状态模型，其中长期存在的进程计算、通信和修改本地状态，而不是数据流模型，在数据流模型中任何进程仅基于输入数据计算并产生由其他进程输入的输出数据。

Pregel was inspired by the Bulk Synchronous Parallel model [45], which provides its synchronous superstep model of computation and communication. There have been a number of general BSP library implementations, for example the Oxford BSP Library [38], Green BSP library [21], BSPlib [26] and Paderborn University BSP library [6]. They vary in the set of communication primitives provided, and in how they deal with distribution issues such as reliability (machine failure), load balancing, and synchronization. To our knowledge, the scalability and fault-tolerance of BSP implementations has not been evaluated beyond several dozen machines, and none of them provides a graph-specific API.
Pregel的灵感来自批量同步并行模型[45]，它提供了同步superstep计算和通信模型。已经有一些通用的BSP库实现，例如Oxford BSP Library[38]、Green BSP library[21]、BSPlib[26]和Paderborn University BSP library[6]。它们在提供的通信原语集以及处理分布问题（如可靠性（机器故障）、负载平衡和同步）的方式上有所不同。据我们所知，BSP实现的可扩展性和容错性尚未在超过几十台机器上进行评估，而且它们都没有提供特定于图的API。

The closest matches to Pregel are the Parallel Boost Graph Library and CGMgraph. The Parallel BGL [22, 23] specifies several key generic concepts for defining distributed graphs, provides implementations based on MPI [18], and implements a number of algorithms based on them. It attempts to maintain compatibility with the (sequential) BGL [43] to facilitate porting algorithms. It implements property maps to hold information associated with vertices and edges in the graph, using ghost cells to hold values associated with remote components. This can lead to scaling problems if reference to many remote components is required. Pregel uses an explicit message approach to acquiring remote information and does not replicate remote values locally. The most critical difference is that Pregel provides fault-tolerance to cope with failures during computation, allowing it to function in a huge cluster environment where failures are common, e.g., due to hardware failures or preemption by higher-priority jobs.
与Pregel最接近的是Parallel Boost Graph Library和CGMgraph。Parallel BGL[22, 23]指定了几个用于定义分布式图的关键通用概念，提供了基于MPI[18]的实现，并基于它们实现了一些算法。它尝试保持与（顺序）BGL[43]的兼容性以方便移植算法。它实现属性映射来保存与图中顶点和边关联的信息，使用ghost cells来保存与远程组件关联的值。如果需要引用许多远程组件，这可能导致扩展问题。Pregel使用显式消息方法来获取远程信息，不在本地复制远程值。最关键的区别是Pregel提供容错以应对计算期间发生的故障，允许它在巨大的集群环境中运行，其中故障是常见的，例如由于硬件故障或被更高优先级作业抢占。

CGMgraph [8] is similar in concept, providing a number of parallel graph algorithms using the Coarse Grained Multicomputer (CGM) model based on MPI. Its underlying distribution mechanisms are much more exposed to the user, and the focus is on providing implementations of algorithms rather than an infrastructure to be used to implement them. CGMgraph uses an object-oriented programming style, in contrast to the generic programming style of Parallel BGL and Pregel, at some performance cost.
CGMgraph[8]在概念上类似，使用基于MPI的粗粒度多计算机（CGM）模型提供多个并行图算法。它的底层分布机制更多地暴露给用户，重点是提供算法实现而不是用作实现的基础设施。CGMgraph使用面向对象的编程风格，与Parallel BGL和Pregel的通用编程风格形成对比，以一些性能成本为代价。

Other than Pregel and Parallel BGL, there have been few systems reporting experimental results for graphs at the scale of billions of vertices. The largest have reported results from custom implementations of s-t shortest path, rather than from general frameworks. Yoo et al [46] report on a BlueGene/L implementation of breadth-first search (s-t shortest path) on 32,768 PowerPC processors with a high-performance torus network, achieving 1.5 seconds for a Poisson distributed random graph with 3.2 billion vertices and 32 billion edges. Bader and Madduri [2] report on a Cray MTA2 implementation of a similar problem on a 10 node, highly multithreaded system, achieving .43 seconds for a scale-free R-MAT random graph with 134 million vertices and 805 million edges. Lumsdaine et al [31] compare a Parallel BGL result on a x86-64 Opteron cluster of 200 processors to the BlueGene/L implementation, achieving .43 seconds for an Erdős-Renyi random graph of 4 billion vertices and 20 billion edges. They attribute the better performance to ghost cells, and observe that their implementation begins to get worse performance above 32 processors.
除Pregel和Parallel BGL外，很少有系统报告数十亿顶点规模的图实验结果。最大的结果来自s-t最短路径的定制实现，而不是通用框架。Yoo等[46]报告了在32,768个PowerPC处理器上使用高性能torus网络实现广度优先搜索（s-t最短路径），在具有32亿顶点和320亿边的Poisson分布随机图上达到1.5秒。Bader和Madduri[2]报告了在10节点高度多线程系统Cray MTA2上实现类似问题，在具有1.34亿顶点和8.05亿边的无标度R-MAT随机图上达到0.43秒。Lumsdaine等[31]将Parallel BGL在200处理器x86-64 Opteron集群上的结果与BlueGene/L实现进行比较，在具有40亿顶点和200亿边的Erdős-Renyi随机图上达到0.43秒。他们将更好的性能归因于ghost cells，并观察到他们的实现在32个处理器以上开始变得更差。

Results for the single-source shortest paths problem on an Erdős-Renyi random graph with 256 million vertices and uniform out-degree 4, using the ∆-stepping algorithm, are reported for the Cray MTA-2 (40 processors, 2.37 sec, [32]), and for Parallel BGL on Opterons (112 processors, 35 sec., [31]). The latter time is similar to our 400-worker result for a binary tree with 1 billion nodes and edges. We do not know of any reported SSSP results on the scale of our 1 billion vertex and 127.1 billion edge log-normal graph.
在具有2.56亿顶点和均匀出度4的Erdős-Renyi随机图上使用∆-stepping算法的单源最短路径问题的结果被报告用于Cray MTA-2（40处理器，2.37秒，[32]）和用于Opterons上的Parallel BGL（112处理器，35秒，[31]）。后者的时间类似于我们对具有10亿节点和边的二叉树的400-worker结果。我们不知道有任何报告的SSSP结果达到我们10亿顶点和1271亿边对数正态图的规模。

Another line of research has tackled use of external disk memory to handle huge problems with single machines, e.g., [33, 36], but these implementations require hours for graphs of a billion vertices.
另一条研究路线致力于使用外部磁盘内存来处理单机上的巨大问题，例如[33, 36]，但这些实现需要数小时处理十亿顶点的图。

8. CONCLUSIONS AND FUTURE WORK
8. 结论和未来工作

The contribution of this paper is a model suitable for large-scale graph computing and a description of its production quality, scalable, fault-tolerant implementation.
本文的贡献是一个适合大规模图计算的模型及其生产质量、可扩展、容错实现的描述。

Based on the input from our users we think we have succeeded in making this model useful and usable. Dozens of Pregel applications have been deployed, and many more are being designed, implemented, and tuned. The users report that once they switch to the "think like a vertex" mode of programming, the API is intuitive, flexible, and easy to use. This is not surprising, since we have worked with early adopters who influenced the API from the outset. For example, aggregators were added to remove limitations users found in the early Pregel model. Other usability aspects of Pregel motivated by user experience include a set of status pages with detailed information about the progress of Pregel programs, a unittesting framework, and a single-machine mode which helps with rapid prototyping and debugging.
基于用户的输入，我们认为我们成功地将这个模型变得有用和可用。数十个Pregel应用程序已被部署，还有更多正在设计、实施和调优。用户报告说，一旦他们切换到"像顶点一样思考"的编程模式，API就很直观、灵活和易于使用。这并不奇怪，因为我们与早期采用者合作，他们从一开始就影响了API。例如，聚合器被添加以消除用户在早期Pregel模型中发现的限制。其他由用户体验驱动的Pregel可用性方面包括一组状态页面，提供关于Pregel程序进度的详细信息、一个单元测试框架和一台有助于快速原型设计和调试的单机器模式。

The performance, scalability, and fault-tolerance of Pregel are already satisfactory for graphs with billions of vertices. We are investigating techniques for scaling to even larger graphs, such as relaxing the synchronicity of the model to avoid the cost of faster workers having to wait frequently at inter-superstep barriers.
Pregel的性能、可扩展性和容错性对于数十亿顶点的图已经令人满意。我们正在研究扩展到更大图的技术，例如放宽模型的同步性以避免更快的worker经常在superstep间屏障等待的成本。

Currently the entire computation state resides in RAM. We already spill some data to local disk, and will continue in this direction to enable computations on large graphs when terabytes of main memory are not available.
目前整个计算状态驻留在RAM中。我们已经将一些数据溢出到本地磁盘，并将继续朝这个方向努力，以在不可获得数TB主内存时启用大型图的计算。

Assigning vertices to machines to minimize inter-machine communication is a challenge. Partitioning of the input graph based on topology may suffice if the topology corresponds to the message traffic, but it may not. We would like to devise dynamic re-partitioning mechanisms.
将顶点分配给机器以最小化机器间通信是一个挑战。如果拓扑与消息流量相对应，则基于拓扑的输入图分区可能就足够了，但可能不够。我们希望设计动态重新分区机制。

Pregel is designed for sparse graphs where communication occurs mainly over edges, and we do not expect that focus to change. Although care has been taken to support high fan-out and fan-in traffic, performance will suffer when most vertices continuously send messages to most other vertices. However, realistic dense graphs are rare, as are algorithms with dense communication over a sparse graph. Some such algorithms can be transformed into more Pregel-friendly variants, for example by using combiners, aggregators, or topology mutations, and of course such computations are difficult for any highly distributed system.
Pregel专为稀疏图设计，其中通信主要沿边发生，我们预计这种重点不会改变。尽管已采取措施支持高扇出和扇入流量，但当大多数顶点持续向大多数其他顶点发送消息时，性能会下降。然而，现实的密集图很少见，稀疏图上密集通信的算法也很少见。一些这样的算法可以转换为更友好的Pregel变体，例如通过使用组合器、聚合器或拓扑突变，当然这样的计算对任何高度分布式系统都是困难的。

A practical concern is that Pregel is becoming a piece of production infrastructure for our user base. We are no longer at liberty to change the API without considering compatibility. However, we believe that the programming interface we have designed is sufficiently abstract and flexible to be resilient to the further evolution of the underlying system.
一个实际的问题是Pregel正在成为我们用户群的生产基础设施。我们不再能自由地改变API而不考虑兼容性。然而，我们相信我们设计的编程接口足够抽象和灵活，能够承受底层系统的进一步发展。