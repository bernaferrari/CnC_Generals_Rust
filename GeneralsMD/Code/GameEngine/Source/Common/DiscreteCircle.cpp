// FILE: DiscreteCircle.cpp ////////////////////////////////////////////////////
//-----------------------------------------------------------------------------
//                                                                          
//                       EA Pacific.                          
//                                                                          
//                       Confidential Information                           
//                Copyright (C) 2002 - All Rights Reserved                  
//                                                                          
//-----------------------------------------------------------------------------
//
// Project:   RTS3
//
// File name: DiscreteCircle.cpp
//
// Created:   John McDonald, September 2002
//
// Desc:      ???
//
//-----------------------------------------------------------------------------

#include "PreRTS.h"
#include "Common/DiscreteCircle.h"

//-------------------------------------------------------------------------------------------------
DiscreteCircle::DiscreteCircle(Int xCenter, Int yCenter, Int radius)
{
	m_yPos = yCenter;
	m_yPosDoubled = (yCenter << 1);
	m_edges.reserve(radius << 1);	// largest that it should ever be.

	generateEdgePairs(xCenter, yCenter, radius);
	removeDuplicates();
}

//-------------------------------------------------------------------------------------------------
void DiscreteCircle::drawCircle(ScanlineDrawFunc functionToDrawWith, void *parmToPass)
{
	for (VecHorzLine::const_iterator it = m_edges.begin(); it != m_edges.end(); ++it) {
		(functionToDrawWith)(it->xStart, it->xEnd, it->yPos, parmToPass);
		if (it->yPos != m_yPos) {
			(functionToDrawWith)(it->xStart, it->xEnd, m_yPosDoubled - it->yPos, parmToPass);
		}
	}
}

//-------------------------------------------------------------------------------------------------
void DiscreteCircle::generateEdgePairs(Int xCenter, Int yCenter, Int radius)
{
	// Uses Bresenham to generate points.
	Int x = 0;
	Int y = radius;
	Int d = (1 - radius) << 1;

	while (y >= 0) {
		HorzLine hl;
		hl.xStart = xCenter - x;
		hl.xEnd		= xCenter + x;
		hl.yPos		= yCenter + y;
		m_edges.push_back(hl);
		
		if (d + y > 0) {
			--y;
			d -= ((y << 1) - 1);
		} 

		if (x > d) {
			++x;
			d += ((x << 1) + 1);
		}
	}
}

//-------------------------------------------------------------------------------------------------
void DiscreteCircle::removeDuplicates()
{
	VecHorzLineIt it, nextIt;
	for ( it = m_edges.begin(); it != m_edges.end(); /* empty */) {
		nextIt = it;
		++nextIt;
		if (nextIt == m_edges.end()) {
			break;
		}

		if (it->yPos == nextIt->yPos) {
			it = m_edges.erase(it);
		} else { 
			++it;
		}
	}
}