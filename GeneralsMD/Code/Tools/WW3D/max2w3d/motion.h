#ifndef MOTION_H
#define MOTION_H


#ifndef ALWAYS_H
#include "always.h"
#endif

#include <Max.h>

#ifndef HIERSAVE_H
#include "hiersave.h"
#endif

#ifndef PROGRESS_H
#include "progress.h"
#endif

#ifndef CHUNKIO_H
#include "chunkio.h"
#endif

#ifndef VECTOR_H
#include "vector.h"
#endif

#ifndef LOGDLG_H
#include "logdlg.h"
#endif

struct W3dExportOptionsStruct;


class MotionClass
{
public:

	MotionClass	(	
						IScene * scene,
						INode * rootnode,
						HierarchySaveClass * basepose,
					   W3dExportOptionsStruct & options,
						int	framerate,
						Progress_Meter_Class * meter,
                  HWND	MaxHwnd,
						char * name,
						Matrix3 & offset = Matrix3(1)		// matrix to bring current object space into
																	// base object space (for damage animations)
					);

	MotionClass	(	
						IScene * scene,
						INodeListClass * rootlist,
						HierarchySaveClass * basepose,
					   W3dExportOptionsStruct & options,
						int	framerate,
						Progress_Meter_Class * meter,
                  HWND MaxHwnd,
						char * name,
						Matrix3 & offset = Matrix3(1)		// matrix to bring current object space into
																	// base object space (for damage animations)
					);

	~MotionClass(void);

	bool Save(ChunkSaveClass & csave);

private:

	IScene *						Scene;
	INode *						RootNode;
	INodeListClass *			RootList;
	HierarchySaveClass *		BasePose;
	int							StartFrame;
	int							EndFrame;
	int							NumFrames;
	int							FrameRate;

	bool							ReduceAnimation;
	int							ReduceAnimationPercent;

	bool							CompressAnimation;
	int							CompressAnimationFlavor;
	float							CompressAnimationTranslationError;
	float							CompressAnimationRotationError;
	
	Progress_Meter_Class *	Meter;
	char							Name[W3D_NAME_LEN];
	Matrix3						Offset;

	// 2D array of matrices, one per node per frame
	Matrix3 * *					MotionMatrix;
	
	// 2D array of euler angles, one per node per frame
	Point3 * *					EulerDelta;
	
	// Visibility bits, one bit per node per frame
	BooleanVectorClass *		VisData;

	// Movement bits, one bit per node per frame, to designate a movement as interpolated, or not
	BooleanVectorClass *		BinMoveData;
	
	// flag for each node in the base pose, indicating
	// whether the node actually appeard in the max scene
	// being exported.
	BooleanVectorClass		NodeValidFlags;

	void			compute_frame_motion(int frame);
	void			set_motion_matrix(int node,int frame,const Matrix3 & motion);
	Matrix3		get_motion_matrix(int node,int frame);
	void			set_eulers(int node,int frame,float x,float y,float z);
	Point3		get_eulers(int node,int frame);
	void			set_visibility(int node,int frame,bool visible);
	bool			get_visibility(int node,int frame);
	void			set_binary_movement(int node,int frame,bool visible);
	bool			get_binary_movement(int node,int frame);

	bool			save_header(ChunkSaveClass & csave);
	bool			save_channels(ChunkSaveClass & csave);

	void			init(void);

};


#endif /*MOTION_H*/