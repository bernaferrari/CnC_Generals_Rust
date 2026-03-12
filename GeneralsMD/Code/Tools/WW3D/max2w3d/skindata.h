#ifndef SKINDATA_H
#define SKINDATA_H

#include "Max.h"
#include "namedsel.h"

/*
** InfluenceStruct - structure which stores the bone 
** influence information for a single vertex.
*/
struct InfluenceStruct
{
	/*
	** vertices can be influenced by up to two bones.
	*/
	int		BoneIdx[2];
	float		BoneWeight[2];

	InfluenceStruct(void) { BoneIdx[0] = -1; BoneIdx[1] = -1; BoneWeight[0] = 1.0f; BoneWeight[1] = 0.0f; }
	
	void Set_Influence(int boneidx) {
		// TODO: make this actually let you set two bones with
		// weighting values.  Need UI to furnish this info...
		BoneIdx[0] = boneidx;
	}
};


/*
** SkinDataClass - a class which contains the bone influence data
** for the modifier.  One of these will be hung off of the 
** ModContext...
*/
class SkinDataClass : public LocalModData
{

public:

	SkinDataClass(void) { Held = FALSE; Valid = FALSE; }

	SkinDataClass(Mesh *mesh)
	{
		VertSel = mesh->vertSel;
		VertData.SetCount(mesh->getNumVerts());
		for (int i=0; i<VertData.Count(); i++) {
			VertData[i].BoneIdx[0] = VertData[i].BoneIdx[1] = -1;
			VertData[i].BoneWeight[0] = 1.0f;
			VertData[i].BoneWeight[1] = 0.0f;
		}
		Valid = TRUE;
		Held = FALSE;
	}

	void Invalidate() { Valid = FALSE; }

	BOOL IsValid() { return Valid; }

	void Validate(Mesh *mesh)
	{
		if (!Valid)
		{
			VertSel.SetSize(mesh->vertSel.GetSize(),1);
			VertData.SetCount(mesh->getNumVerts());
			Valid = TRUE;
		}
	}

	virtual LocalModData * Clone(void) 
	{ 
		SkinDataClass * newdata = new SkinDataClass();
		newdata->VertSel = VertSel;
		newdata->VertData = VertData;
		return newdata;
	}

	void Add_Influence(int boneidx) 
	{
		/*
		** Make this INode influence all currently selected vertices
		*/
		for (int i=0; i<VertData.Count(); i++) {
			if (VertSel[i]) {
				VertData[i].Set_Influence(boneidx);
			}
		}
	}

	IOResult Save(ISave *isave);
	IOResult Load(ILoad *iload);

public:

	BOOL							Valid;
	BOOL							Held;
	
	/*
	** Current selection
	*/
	BitArray						VertSel;
	
	/*
	** Named selection sets
	*/
	NamedSelSetList			VertSelSets;

	/*
	** Vertex influence data
	*/
	Tab<InfluenceStruct>		VertData;

	/*
	** Load/Save chunk ID's
	*/
	enum {
		FLAGS_CHUNK = 				0x0000,
		VERT_SEL_CHUNK = 			0x0010,	
		NAMED_SEL_SETS_CHUNK =	0x0020,
		INFLUENCE_DATA_CHUNK = 	0x0030
	};

};


#endif
