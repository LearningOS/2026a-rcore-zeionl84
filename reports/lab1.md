# lab1 实验报告

>
> 实验分支：ch3　章节：多道程序与分时多任务

## 一、实现的功能

本次实验在 ch3 模板的基础上补完了 sys_trace系统调用，并完成相应的了系统调用次数的统计：
trace_request 为 0 时读取当前任务指定地址的一个字节；为 1 时把 data 的最低 8 位写入指定地址；为 2 时返回当前任务对编号 id 的系统调用的调用次数；其余取值返回 -1。
统计用的计数数组放在任务控制模块内，每个任务一份，全局有问题但不知道为什么；
按照要求，本次 trace 调用自身也会被计入。
多道程序加载、yield 让出 CPU、时钟中断抢占等部分沿用模板，以防止可能的意外（虽然在编写过程中尝试过创新，但出现无法启动的问题，因此退回到模板）。


## 二、简答作业

### 2.1 三个 bad 测例的出错行为

使用的 SBI 及版本：仓库自带的 RustSBI 固件 `bootloader/rustsbi-qemu.bin`，版本 0.4.1。

实际启动时看到的版本信息：

```text
[RustSBI] INFO  - Hello RustSBI!
[RustSBI] INFO  - RustSBI version 0.4.1
```

观察到的出错行为：

1. `ch2b_bad_address`：用户程序试图向地址 `0x0` 写入数据，触发 Store Page Fault。内核打印
   `[kernel] PageFault in application, bad addr = 0x0, bad instruction = 0x...` 并提示 killed it，
   随后该任务被置为已退出，内核继续调度其它任务。
2. `ch2b_bad_instructions`：用户程序执行非法指令，触发 IllegalInstruction 异常。内核打印
   `[kernel] IllegalInstruction in application, kernel killed it.`
3. `ch2b_bad_register`：用户程序在 U 态执行 `csrw` 一类指令去写 `sstatus` 等 S 态特权寄存器，同样触发 IllegalInstruction，内核打印
   `[kernel] IllegalInstruction in application, kernel killed it.`

说明：这三例共同说明程序进入 U 态后确实受硬件限制——S 态特权寄存器不可访问，非法指令无法执行，访问无权访问的地址会产生页错误。而且内核没有整体崩溃，只是打印信息并杀掉出错的任务，任务级隔离是有效的（没有完整的查看全部，无法叙述逻辑）。

（下面是 `BASE=2` 时三个 bad 测例的实际输出）

```text
[kernel] PageFault in application, bad addr = 0x0, bad instruction = 0x804003a4, kernel killed it.
[kernel] IllegalInstruction in application, kernel killed it.
[kernel] IllegalInstruction in application, kernel killed it.
```

### 2.2 __alltraps 与 __restore

**(1) 刚进入 __restore 时 sp 代表什么值？__restore 的两种使用情景？**

`sp` 代表内核栈上刚刚分配并填好的 `TrapContext` 的起始地址。

两种使用情景：

从 trap 返回：`trap_handler()` 处理完异常后调用 `__restore`，恢复被打断任务的上下文，`sret` 返回用户态；
新任务第一次被调度：`TaskContext::goto_restore(init_app_cx(i))` 把新任务的 `ra` 设为 `__restore`、`sp` 设为已构造好 `TrapContext` 的内核栈位置，`__switch` 切过去时直接跳进 `__restore`，从而第一次进入用户态。

**(2) L43–L48 特殊处理了哪些寄存器？这些值对进入用户态有何意义？**

这几行恢复的是 `sstatus`、`sepc`和 `sscratch`。

`sstatus`：决定 `sret` 之后的状态。SPP 位决定返回哪个特权级（0 即 U 态），SPIE 位决定 `sret` 后是否使能中断。
`sepc`：`sret` 会把 PC 设为它，决定返回到用户程序的哪条指令（系统调用后是 `ecall` 的下一条）。
`sscratch`：暂存用户栈指针，供 L60 与 `sp` 交换使用。

意义：sstatus 决定"回去状态"，sepc 决定"回到哪里"，sscratch 决定"用哪个栈"，三者缺一就无法正确回到 U 态。

**(3) L50–L56 为何跳过 x2 和 x4？**

`x2` 是 `sp`。用户栈指针在 `__alltraps` 里并没有直接保存，而是把 `sscratch` 中的用户栈指针写进 2\*8 槽位；若在此处提前 `ld x2`，`sp` 会立刻变成用户栈，但后面 L58 还要用 `sp` 释放内核栈上的 `TrapContext`、L60 还要用 `sp` 与 `sscratch` 交换，所以必须留到 L60 一并完成。
`x4` 是 `tp`。模板里用户程序不使用它，`__alltraps` 根本没有保存，也就不需要恢复；而且内核可能用 `tp` 指向自己的 TLS，强行覆盖会出问题。

**(4) L60 之后 sp 和 sscratch 分别有什么意义？**

执行 `csrrw sp, sscratch, sp` 之后：

`sp` = 用户栈指针，回到用户自己的栈；
`sscratch` = 内核栈指针（此时 `TrapContext` 已释放，指向内核栈顶）。

这样下次从 U 态陷入时，`__alltraps` 的第一条 `csrrw` 又能把两者换回来，重新取得内核栈。

**(5) __restore 中状态切换发生在哪条指令？为何执行后会进入用户态？**

发生在最后一条 `sret`。它按 `sstatus.SPP` 恢复特权级，把 `sepc` 装入 PC，并按 `sstatus.SPIE` 恢复中断使能。执行完这条指令，CPU 已经以 U 态从 `sepc` 处取指针，因此进入了用户程序。

**(6) L13 之后 sp 和 sscratch 分别有什么意义？**

`csrrw sp, sscratch, sp` 之后：

- `sp` = 内核栈指针（本任务内核栈中用于放 `TrapContext` 的位置），后续内核代码都在内核栈上运行；
- `sscratch` = 用户栈指针，先暂存在这里，等 L34–L35 再取出来存进 `TrapContext` 的 x2 槽位。

**(7) 从 U 态进入 S 态是哪条指令发生的？**

系统调用是用户程序执行 `ecall` 这一条指令触发的；
其它同步异常（非法指令、访存错误等）由当时正在执行的那条指令引发；
异步中断则没有对应指令，由硬件检测到后自动完成。不论哪种原因，硬件都会把 PC 置为 `stvec` 指向的入口，因此进入 S 态后执行的第一条指令就是 `__alltraps` 里的 L13 `csrrw sp, sscratch, sp`。

## 三、运行与测试记录

### 3.1 编译与运行

`make -C os run CHAPTER=3 TEST=3 BASE=2` 的实际输出（截取关键部分）：

```text
[RustSBI] INFO  - Redirecting hart 0 to 0x00000080200000 in Supervisor mode.
[kernel] Hello, world!
[kernel] PageFault in application, bad addr = 0x0, bad instruction = 0x804003a4, kernel killed it.
[kernel] IllegalInstruction in application, kernel killed it.
[kernel] IllegalInstruction in application, kernel killed it.
Hello, world from user mode program!
...
3^200000 = 871008973(MOD 998244353)
Test power_3 OK!
...
5^140000 = 386471875(MOD 998244353)
Test power_5 OK!
...
7^160000 = 667897727(MOD 998244353)
Test power_7 OK!
get_time OK! 29
current time_msec = 30
AAAAAAAAAA [1/5]
BBBBBBBBBB [1/5]
CCCCCCCCCC [1/5]
...
AAAAAAAAAA [5/5]
BBBBBBBBBB [5/5]
CCCCCCCCCC [5/5]
Test write A OK!
Test write B OK!
Test write C OK!
time_msec = 130 after sleeping 100 ticks, delta = 100ms!
Test sleep1 passed!
string from task trace test

Test trace OK!
Test sleep OK!
[kernel] Panicked at src/task/mod.rs:145 All applications completed!
```

### 3.2 测例检查结果

检查器 `make -C ci-user test CHAPTER=3` 需要另外克隆 rCore-Tutorial-Checker 仓库，本地网络拉取卡住了，貌似是校园网问题，用手机连不上网络，所以改为直接用 `ci-user/check/ch3.py` 里那 7 条检查项，对 `BASE=2` 的完整运行输出做同样的正则匹配，结果如下：

```text
[PASS] found <get_time OK! (\d+)>
[PASS] found <Test sleep OK!>
[PASS] found <current time_msec = (\d+)>
[PASS] found <time_msec = (\d+) after sleeping (\d+) ticks, delta = (\d+)ms!>
[PASS] found <Test sleep1 passed!>
[PASS] found <string from task trace test>
[PASS] found <Test trace OK!>

Test passed: 7/7
```

### 3.3 自己观察到的现象

- A/B/C 三个程序是否交替输出，说明 `yield` 与时钟抢占是否正常：三个程序严格按 A → B → C 的顺序轮流打印，每轮各输出 10 个字符并带 `[i/5]` 计数，说明 `yield` 主动让出和时间片抢占都工作正常。
- `ch3_sleep1` 打印的 delta 与 100ms 差多少：`time_msec = 130 after sleeping 100 ticks, delta = 100ms!`，实际 delta 正好是 100ms，没有偏差（`get_time OK! 29` 与 `current time_msec = 30` 也能看出时间在正常前进）。
- 全部任务结束时是否出现 `All applications completed!`：出现了，输出为 `[kernel] Panicked at src/task/mod.rs:145 All applications completed!`。

## 四、荣誉准则

> 按课程要求，将填写了《你的说明》的下述内容拷贝到实验报告中。

1. 在完成本次实验的过程（含此前学习的过程）中，我曾分别与 **以下各位** 就（与本次实验相关的）以下方面做过交流，还在代码中对应的位置以注释形式记录了具体的交流对象及内容：

> 《你交流的对象说明》
>
> DeepSeek

2. 此外，我也参考了 **以下资料**，还在代码中对应的位置以注释形式记录了具体的参考来源及内容：

> 《你参考的资料说明》
>
> deepseek
>vscode自带的变量名推荐与对应的rust检查器

3. 我独立完成了本次实验除以上方面之外的所有工作，包括代码与文档。我清楚地知道，从以上方面获得的信息在一定程度上降低了实验难度，可能会影响起评分。

4. 我从未使用过他人的代码，不管是原封不动地复制，还是经过了某些等价转换。我未曾也不会向他人（含此后各届同学）复制或公开我的实验代码，我有义务妥善保管好它们。我提交至本实验的评测系统的代码，均无意于破坏或妨碍任何计算机系统的正常运转。

我清楚地知道，以上情况均为本课程纪律所禁止，若违反，对应的实验成绩将按 "-100" 分计。


## 五、对实验设计的看法（可选）

my:(

## 六：说明

本文档由复制任务说明，我（本人）填写内容完成。所有代码内容均为本人加参考课程内容完成，部分使用可能不规范，或过度，在测试中无影响，如果有问题会因为过大而产生明显错误，故没有添加报错。