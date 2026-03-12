#pragma once

#ifndef __BENCHMARK_H__
#define __BENCHMARK_H__

#ifdef __cplusplus
extern "C" {
#endif

int RunBenchmark(int argc, char *argv[], float *floatResult, float *intResult, float *memResult);

#ifdef __cplusplus
}
#endif

#endif