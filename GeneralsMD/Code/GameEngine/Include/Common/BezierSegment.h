// BezierSegment.h ////////////////////////////////////////////////////////////////////////////////
// John K McDonald, Jr.
// September 2002
// DO NOT DISTRIBUTE

#pragma once
#ifndef __BEZIERSEGMENT_H__
#define __BEZIERSEGMENT_H__

#include <D3DX8Math.h>
#include "Common/STLTypeDefs.h"

#define USUAL_TOLERANCE 1.0f

class BezierSegment
{
	protected:
		static const D3DXMATRIX s_bezBasisMatrix;
		Coord3D m_controlPoints[4];

	public:	// Constructors
		BezierSegment();
		BezierSegment(Real x0, Real y0, Real z0,
									Real x1, Real y1, Real z1,
									Real x2, Real y2, Real z2,
									Real x3, Real y3, Real z3);

		BezierSegment(Real cp[16]);


		BezierSegment(const Coord3D& cp0, 
									const Coord3D& cp1, 
									const Coord3D& cp2, 
									const Coord3D& cp3);

		BezierSegment(Coord3D cp[4]);

	public:
		void evaluateBezSegmentAtT(Real tValue, Coord3D *outResult) const;
		void getSegmentPoints(Int numSegments, VecCoord3D *outResult) const;
		Real getApproximateLength(Real withinTolerance = USUAL_TOLERANCE) const;

		void splitSegmentAtT(Real tValue, BezierSegment &outSeg1, BezierSegment &outSeg2) const;

	public:	// He get's friendly access.
		friend class BezFwdIterator;
};

#endif /* __BEZIERSEGMENT_H__ */
