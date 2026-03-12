#ifndef VXLDBG_H
#define VXLDBG_H

#ifndef ALWAYS_H
#include "always.h"
#endif

#include <Max.h>

#ifndef SIMPDIB_H
#include "simpdib.h"
#endif

#ifndef VXL_H
#include "vxl.h"
#endif


class VoxelDebugWindowClass
{
public:

	VoxelDebugWindowClass(VoxelClass * vxl);
	~VoxelDebugWindowClass(void);

	void	Display_Window(void);
	bool	Dialog_Proc(HWND hWnd,UINT message,WPARAM wParam,LPARAM);

private:

	int						CurLayer;

	SimpleDIBClass *		Bitmap;
	VoxelClass *			Voxel;
	HWND						WindowHWND;
	HWND						ViewportHWND;
	ISpinnerControl *		LayerSpin;
 
	void update_display(void);
};



#endif
