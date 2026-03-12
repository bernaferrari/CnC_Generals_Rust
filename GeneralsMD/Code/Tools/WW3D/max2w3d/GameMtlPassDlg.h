#ifndef GAMEMTLPASSDLG_H
#define GAMEMTLPASSDLG_H

#include <Max.h>

class GameMtl;
class GameMtlFormClass;

/*
** The GameMtlPassDlg will contain a Tab Control which switches between
** editing the VertexMaterial parameters, the Shader parameters and the
** Texture parameters.
*/
class GameMtlPassDlg: public ParamDlg 
{
public:
	
	GameMtlPassDlg(HWND hwMtlEdit, IMtlParams *imp, GameMtl *m,int pass); 
	~GameMtlPassDlg();

	BOOL					DialogProc(HWND hDlg, UINT message, WPARAM wParam, LPARAM lParam);

	void					Invalidate();		
	void					UpdateMtlDisplay()						{ IParams->MtlChanged(); }

	void					ReloadDialog();
	Class_ID				ClassID();
	void					SetThing(ReferenceTarget* target);
	ReferenceTarget *	GetThing()									{ return (ReferenceTarget *)TheMtl; }
	void					DeleteThis()								{ delete this;  }	
	void					SetTime(TimeValue t);
	void					ActivateDlg(BOOL onOff);

	enum { PAGE_COUNT = 3 };

	////////////////////////////////////////////////////////////////////////
	// Material dialog interface
	////////////////////////////////////////////////////////////////////////
	IMtlParams *		IParams;			// interface to the material editor
	GameMtl *			TheMtl;			// current mtl being edited.

	////////////////////////////////////////////////////////////////////////
	// Windows handles
	////////////////////////////////////////////////////////////////////////
	HWND					HwndEdit;		// window handle of the materials editor dialog
	HWND					HwndPanel;		// Rollup parameters panel

	////////////////////////////////////////////////////////////////////////
	// Variables
	////////////////////////////////////////////////////////////////////////
	int					PassIndex;
	int					CurPage;
	BOOL					Valid;

	GameMtlFormClass*	Page[PAGE_COUNT];
};

#endif