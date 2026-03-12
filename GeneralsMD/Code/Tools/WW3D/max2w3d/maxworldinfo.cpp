#include "maxworldinfo.h"
#include "geometryexporttask.h"

/*
** Get_Shared_Vertex_Normal
** Loops through all the other meshes in the world and builds a vertex normal for
** the verticies that share the same space and are part of the same smoothing group.
*/
Vector3 MaxWorldInfoClass::Get_Shared_Vertex_Normal (Vector3 pos, int smgroup)
{
	Point3 normal(0,0,0);
	Point3 world_pos = ExportTrans * Point3(pos.X,pos.Y,pos.Z);

	//
	//	Loop through all the meshes in the world and see which ones contain
	// verticies that share the same space and are part of the same smoothing group.
	//
	for(unsigned int index = 0; index < MeshList.Count(); index ++) {
		GeometryExportTaskClass * task = MeshList[index];
		if (task != CurrentTask) {
			normal += task->Get_Shared_Vertex_Normal(world_pos,smgroup);			
		}
	}

	return Vector3(normal.x,normal.y,normal.z);
}
