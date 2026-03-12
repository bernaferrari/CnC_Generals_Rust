#ifndef GEOMETRYEXPORTCONTEXT_H
#define GEOMETRYEXPORTCONTEXT_H

#include <Max.h>

class ChunkSaveClass;
class MaxWorldInfoClass;
class HierarchySaveClass;
class INodeListClass;
class Progress_Meter_Class;
struct W3dExportOptionsStruct;

 
/**
** ExportContextClass
** This class encapsulates a bunch of datastructures needed during the geometry export
** process. 
** NOTE: The user must plug in a valid ProgressMeter before each export operation.
*/
class GeometryExportContextClass
{
public:
	GeometryExportContextClass(	char * model_name,
											ChunkSaveClass & csave,
											MaxWorldInfoClass & world_info,
											W3dExportOptionsStruct & options,
											HierarchySaveClass * htree,
											INode * origin,
											INodeListClass * origin_list,
											TimeValue curtime,
											unsigned int *materialColors
										) :
		CSave(csave),
		WorldInfo(world_info),
		Options(options),
		CurTime(curtime),
		HTree(htree),
		OriginList(origin_list),
		Origin(origin),
		OriginTransform(1),
		ProgressMeter(NULL),
		materialColors(materialColors),
		numMaterialColors(0),
		numHouseColors(0),
		materialColorTexture(NULL)
	{
		ModelName = strdup(model_name);
		OriginTransform = Origin->GetNodeTM(CurTime);
	}
	
	~GeometryExportContextClass(void)
	{
		delete[] ModelName;
	}

	char *							ModelName;
	ChunkSaveClass &				CSave;
	MaxWorldInfoClass &			WorldInfo;
	W3dExportOptionsStruct &	Options;
	TimeValue						CurTime;
	HierarchySaveClass *			HTree;
	INodeListClass *				OriginList;

	INode *							Origin;
	Matrix3							OriginTransform;
	Progress_Meter_Class	*		ProgressMeter;
	unsigned int *					materialColors;	///MW: holds all used material colors.
	int								numMaterialColors;	///MW: number of used material colors.
	int								numHouseColors;		///MW: number of used house colors
	char	*						materialColorTexture; //MW: texture to hold material colors
};



#endif //GEOMETRYEXPORTCONTEXT_H

