/** FrameMetrics.h */

#pragma once

#ifndef __FRAMEMETRICS_H
#define __FRAMEMETRICS_H

#include "Lib/BaseType.h"
#include "GameNetwork/NetworkDefs.h"

class FrameMetrics {
public:
	FrameMetrics();
	virtual ~FrameMetrics();

	void init();
	void reset();

	void doPerFrameMetrics(UnsignedInt frame);
	void processLatencyResponse(UnsignedInt frame);
	void addCushion(Int cushion);

	Real getAverageLatency();
	Int getAverageFPS();
	Int getMinimumCushion();

protected:
	// These are used for keeping track of parameters to the run ahead equation.
	// frames per second history variables.
	Real *m_fpsList;								///< A record of how many game logic frames per second there were for the last 60 seconds.
	time_t m_lastFpsTimeThing;																///< The time when the last fps entry started being recorded.
	Int m_fpsListIndex;																				///< Index into the array of the current fps list entry being measured.
	Real m_averageFps;																				///< The current average logic fps, computed just like m_averageLatency below but with the fps numbers.

	// round trip time to packet router variables.
	// The lists are indexed off the frame number of the frame info packet they are associated with.
	// The index used should be the frame number mod the array length.
	Real *m_latencyList;				///< A record of the round trip latencies of the frame info packets to the packet router.  Values in seconds.
	time_t *m_pendingLatencies;																///< The latencies of frame info packets that are "in the air."
	Real m_averageLatency;																		///< The current average latency, this is used to save calculation time.
																														///< When a new latency value is received, the old one is subtracted out and the new
																														///< one is added in.

	// packet arrival cushion variables.
	// Keeps track of the cushion for the incoming commands.
	UnsignedInt m_cushionIndex;															///< The next index to use for the cushion list.
	Int m_minimumCushion;																			///< The average cushion for the history.
};

#endif // __FRAMEMETRICS_H