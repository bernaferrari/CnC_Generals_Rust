#if defined(_MSC_VER)
#pragma once
#endif

#ifndef PREDLOD_H
#define PREDLOD_H

// This file contains the classes which support predictive LOD management
// similar to that outlined in "Adaptive Display Algorithm for Interactive
// Frame Rates During Visualization of Complex Virtual Environments",
// Thomas Funkhouser & Carlo Sequin, SIGGRAPH '93 Proceedings, pp. 247-253.
// To this "pure predictive" LOD we have added distance (actually screensize)
// clamping to control quality degradation.

#include "rendobj.h"
#include "float.h"
#include "vector.h"

class LODHeapNode;

/*
** PredictiveLODOptimizerClass: Class which performs the predictive LOD
** optimization. All the members of this class are static.
*/
class PredictiveLODOptimizerClass {

	public:

		static void		Clear(void);
		static void		Add_Object(RenderObjClass *robj);
		static void		Add_Cost(float cost)								{ TotalCost += cost; }
		static void		Optimize_LODs(float max_cost);
		static float	Get_Total_Cost(void)								{ return TotalCost; }
		static void		Free(void);	// frees all memory

	private:
		static void		AllocVisibleObjArrays(int num_objects);

		static RenderObjClass **	ObjectArray;
		static int						ArraySize;
		static int						NumObjects;
		static float					TotalCost;

		static LODHeapNode *VisibleObjArray1;
		static LODHeapNode *VisibleObjArray2;
		static int VisibleObjArraySize;

};

#endif
