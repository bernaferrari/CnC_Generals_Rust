#if defined(_MSC_VER)
#pragma once
#endif

#include "SnapPoints.h"
#include "chunkio.h"
#include "Max.h"
#include "nodelist.h"
#include "w3d_file.h"


class PointFilterClass : public INodeFilterClass
{
public:
	PointFilterClass(void) { }

	virtual BOOL Accept_Node(INode * node, TimeValue time)
	{
		if (node == NULL) return FALSE;
		Object * obj = node->EvalWorldState(time).obj;
		if (obj == NULL) return FALSE;
		
		if 
		(
			obj->ClassID() == Class_ID(POINTHELP_CLASS_ID,0) &&
			!node->IsHidden()
		) 
		{
			return TRUE;
		} else {
			return FALSE;
		} 
	}
};


void SnapPointsClass::Export_Points(INode * scene_root,TimeValue time,ChunkSaveClass & csave)
{
	if (scene_root == NULL) return;
	
	PointFilterClass pointfilter;
	INodeListClass pointlist(scene_root,time,&pointfilter);

	if (pointlist.Num_Nodes() > 0) {

		csave.Begin_Chunk(W3D_CHUNK_POINTS);

		for (unsigned int ci=0; ci<pointlist.Num_Nodes(); ci++) {

			W3dVectorStruct vect;
			Point3 pos = pointlist[ci]->GetNodeTM(time).GetTrans();
			vect.X = pos.x;
			vect.Y = pos.y;
			vect.Z = pos.z;
			csave.Write(&vect,sizeof(vect));

		}

		csave.End_Chunk();
	}
}
