#ifndef HLODSAVE_H
#define HLODSAVE_H

#include "always.h"

#include <Max.h>
#include <stdio.h>

#include "w3d_file.h"
#include "progress.h"
#include "chunkio.h"
#include "meshcon.h"


class INodeListClass;
class MeshConnectionsClass;


/**
** HLodSaveClass
** This object takes an array of mesh-connections objects and exports an LOD model
** constructed from them.
*/
class HLodSaveClass
{
public:
	HLodSaveClass (MeshConnectionsClass **connections, int lod_count, TimeValue CurTime,
						char *name, const char *htree_name, Progress_Meter_Class &meter,
						INodeListClass *origin_list);
	~HLodSaveClass (void);

	bool Save (ChunkSaveClass &csave);


protected:

	/*
	** class HLodArrayEntry hold the HLOD tree that we will save out in the Save() method.
	*/
	class HLodArrayEntry
	{
	public:
		W3dHLodArrayHeaderStruct	header;
		W3dHLodSubObjectStruct		*sub_obj;
		int								num_sub_objects;

		HLodArrayEntry (int num_sub_objs = 0)
		{
			sub_obj = NULL;
			num_sub_objects = 0;
			Allocate_Sub_Objects(num_sub_objs);
		}

		~HLodArrayEntry (void)
		{
			if (sub_obj)
			{
				delete sub_obj;
				sub_obj = NULL;
				num_sub_objects = 0;
			}
		}

		bool Allocate_Sub_Objects (int num)
		{
			if (num <= 0) return false;
			num_sub_objects = 0;
			sub_obj = new W3dHLodSubObjectStruct[num];
			if (!sub_obj) return false;
			num_sub_objects = num;
			return true;
		}

		bool operator == (const HLodArrayEntry & that)	{ return false; }
		bool operator != (const HLodArrayEntry & that)	{ return !(*this == that); }
	};

	bool save_header (ChunkSaveClass &csave);
	bool save_lod_arrays (ChunkSaveClass &csave);
	bool save_aggregate_array (ChunkSaveClass & csave);
	bool save_proxy_array(ChunkSaveClass & csave);
	bool save_sub_object_array(ChunkSaveClass & csave, const HLodArrayEntry & array);

	W3dHLodHeaderStruct					header;
	HLodArrayEntry	*						lod_array;
	HLodArrayEntry							aggregate_array;
	HLodArrayEntry							proxy_array;
};



#endif
