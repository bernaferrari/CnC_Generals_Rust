// FILE: GraphDraw.h //////////////////////////////////////////////////////////
//-----------------------------------------------------------------------------
//                                                                          
//                       Electronic Arts Pacific.                          
//                                                                          
//                       Confidential Information                           
//                Copyright (C) 2002 - All Rights Reserved                  
//                                                                          
//-----------------------------------------------------------------------------
//
//	created:	Aug 2002
//
//	Filename: 	GraphDraw.h
//
//	author:		John McDonald
//	
//	purpose:	Contains the functions to queue up and display a single graph for 
//						each frame. Note: This class is presently only intended for use by
//						the Performance timers, all though it could be easily adapted for 
//						other purposes.
//
//-----------------------------------------------------------------------------
///////////////////////////////////////////////////////////////////////////////

#pragma once
#ifndef __GRAPHDRAW_H__
#define __GRAPHDRAW_H__

#include "Common/PerfTimer.h"
#include "Common/STLTypedefs.h"

#ifdef PERF_TIMERS

typedef std::pair<AsciiString, Real> PairAsciiStringReal;
typedef std::vector<PairAsciiStringReal> VecGraphEntries;
typedef VecGraphEntries::iterator VecGraphEntriesIt;

enum { MAX_GRAPH_VALUES = 36 };
enum { BAR_HEIGHT = 14 };
enum { BAR_SPACE = 2 };

class DisplayString;

class GraphDraw
{
	public:
		GraphDraw();
		virtual ~GraphDraw();

		void addEntry(AsciiString str, Real val);
		// Called during begin/end
		void render();
		void clear();

	protected:
		VecGraphEntries m_graphEntries;
		DisplayString *m_displayStrings[MAX_GRAPH_VALUES];
};

extern GraphDraw *TheGraphDraw;


#endif /* PERF_TIMERS */

#endif /* __GRAPHDRAW_H__ */

