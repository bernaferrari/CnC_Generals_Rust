#pragma once

#include "Common/BezierSegment.h"

class BezFwdIterator
{
	protected:
		Int mStep;
		Int mStepsDesired;

		BezierSegment mBezSeg;
		Coord3D mCurrPoint;
		
		Coord3D mDq;	// First Derivative
		Coord3D mDDq;	// Second Derivative
		Coord3D mDDDq;	// Third Derivative

	public:
		BezFwdIterator();
		BezFwdIterator(Int stepsDesired, const BezierSegment *bezSeg);
	
		void start(void);
		Bool done(void);
		const Coord3D& getCurrent(void) const;

		void next(void);
};