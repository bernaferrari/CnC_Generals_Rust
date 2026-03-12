#ifndef MAXWORLDINFO_H
#define MAXWORLDINFO_H


#include <Max.h>
#include "meshbuild.h"
#include "nodelist.h"
#include "vector.h"


class GeometryExportTaskClass;


/**
** MaxWorldInfoClass - Provides information about the max 'world' (or scene)
** This class is used by the plugin to cause the MeshBuilder to smooth normals
** across adjacent meshes.
*/
class MaxWorldInfoClass : public WorldInfoClass
{
	public:
		MaxWorldInfoClass(DynamicVectorClass<GeometryExportTaskClass *> & mesh_list)
			:	MeshList (mesh_list),
				SmoothBetweenMeshes (true),
				CurrentTask(NULL),
				CurrentTime(0)					{ }
		virtual ~MaxWorldInfoClass(void)	{ }

		// Public methods		
		virtual Vector3	Get_Shared_Vertex_Normal(Vector3 pos, int smgroup);
		
		virtual GeometryExportTaskClass *	Get_Current_Task(void) const								{ return CurrentTask; }
		virtual void								Set_Current_Task(GeometryExportTaskClass * task)	{ CurrentTask = task; }

		virtual TimeValue	Get_Current_Time(void) const	{ return CurrentTime; }
		virtual void		Set_Current_Time(TimeValue &time) { CurrentTime = time; }

		virtual Matrix3	Get_Export_Transform(void) const	{ return ExportTrans; }
		virtual void		Set_Export_Transform(const Matrix3 &matrix) { ExportTrans = matrix; }

		virtual void		Allow_Mesh_Smoothing (bool onoff)	{ SmoothBetweenMeshes = onoff; }
		virtual bool		Are_Meshes_Smoothed (void) const		{ return SmoothBetweenMeshes; }
		
	private:

		DynamicVectorClass<GeometryExportTaskClass *> &	MeshList;
		GeometryExportTaskClass *								CurrentTask;
		TimeValue			CurrentTime;
		Matrix3				ExportTrans;
		bool					SmoothBetweenMeshes;
};



#endif