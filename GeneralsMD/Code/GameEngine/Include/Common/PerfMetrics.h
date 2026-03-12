// PerfMetrics.h //////////////////////////////////////////////////////////////////////////////////
// Author: John McDonald, Jr August 2002
// Do Not Distribute

#pragma once

#ifndef __PERFMETRICS_H__
#define __PERFMETRICS_H__

// For load timings
enum { PERFMETRICS_LOAD_STARTFRAME = 0 };
enum { PERFMETRICS_LOAD_STOPFRAME = 1 };

// For engine timings
enum { PERFMETRICS_LOGIC_STARTFRAME = 1 };
enum { PERFMETRICS_LOGIC_STOPFRAME = 1000 };


// For showing metrics
enum { PERFMETRICS_BETWEEN_METRICS = 150 };

#endif /* __PERFMETRICS_H__ */


