// DiscreteCircle.h ///////////////////////////////////////////////////////////////////////////////
// John K McDonald, Jr.
// September 2002
// DO NOT DISTRIBUTE

#pragma once

#ifndef __DISCRETECIRCLE_H__
#define __DISCRETECIRCLE_H__

//-------------------------------------------------------------------------------------------------
/**
	One horizontal line of the circle we are going to generate, the points drawn should be from 
	(xStart, yPos)-(xEnd, yPos), inclusive.
*/
struct HorzLine
{
	Int yPos;
	Int xStart;
	Int xEnd;
};

// Vector and Iterators for the HorzLine struct.
typedef std::vector<HorzLine> VecHorzLine;
typedef VecHorzLine::iterator VecHorzLineIt;

//-------------------------------------------------------------------------------------------------

// Useful if you'd like to not have to deal with the logic of drawing the circle.
typedef void (*ScanlineDrawFunc)(Int xStart, Int xEnd, Int yPos, void *otherParms);

/**
	DiscreteCircle generates a circle centered at xCenter, yCenter, including radius. It generates 
	horizontal segments for the top half of the circle only, so they need to be duplicated for the 
	bottom half.
*/
class DiscreteCircle
{
	VecHorzLine m_edges;	// Should be HorzLines
	Int m_yPos;						// Used to know when to draw the bottom scanline
	Int m_yPosDoubled;		// Used to draw the bottom half of the circle.

	public:
		DiscreteCircle(Int xCenter, Int yCenter, Int radius);
		__inline const VecHorzLine &getEdges(void) const { return m_edges; }
		__inline Int getEdgeCount(void) const { return m_edges.size(); }
		void drawCircle(ScanlineDrawFunc functionToDrawWith, void *parmToPass);
		
	protected:
		void generateEdgePairs(Int xCenter, Int yCenter, Int radius);
		void removeDuplicates();
};

#endif /* __DISCRETECIRCLE_H__ */

