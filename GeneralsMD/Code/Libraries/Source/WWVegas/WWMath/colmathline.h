#ifndef COLMATHLINE_H
#define COLMATHLINE_H

/*
** Inline collision functions dealing with line segments
*/


inline CollisionMath::OverlapType	CollisionMath::Overlap_Test
(
	const Vector3 &		min,
	const Vector3 &		max,
	const LineSegClass & line
)
{
	AABoxClass box;
	box.Init_Min_Max(min,max);
	return CollisionMath::Overlap_Test(box,line);
}




#endif


