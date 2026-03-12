#if defined(_MSC_VER)
#pragma once
#endif

#ifndef WW3D2_STRIP_OPTIMIZER_H__
#define WW3D2_STRIP_OPTIMIZER_H__

#include "always.h"


// strip data =
//
// len = number of vertices in strip
// [vertex indices]
// len
// [vertex indices]
// ..

class StripOptimizerClass
{
public:
	static int* Stripify(const int* tris, int tri_count); // Outputs a set of strips
	static int* Combine_Strips(const int* strips, int strip_count);
	static void Optimize_Strip_Order(int* strips, int strip_count); // Sorts strips for optimal access order
	static void Optimize_Triangle_Order(int* tris, int triangle_count); // Sorts triangles (three indices each) into near-optimal access order

	static int Get_Strip_Index_Count(const int* strips, int strips_count);
};

#endif // WW3D2_STRIP_OPTIMIZER_H__
