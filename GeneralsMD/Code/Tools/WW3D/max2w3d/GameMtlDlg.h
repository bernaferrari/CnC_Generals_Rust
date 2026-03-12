#ifndef GAMEMTLDLG_H
#define GAMEMTLDLG_H

class GameMtl;
class GameMtlPassDlg;


////////////////////////////////////////////////////////////////////////
// GameMtlDlg
//
// Dialog box interface in the material editor for GameMtl
//	This is basically a cannibalized version of the Standard
// Max material's dialog.
//
////////////////////////////////////////////////////////////////////////
class GameMtlDlg: public ParamDlg 
{

public:
	
	////////////////////////////////////////////////////////////////////////
	// Methods
	////////////////////////////////////////////////////////////////////////
	GameMtlDlg(HWND hwMtlEdit, IMtlParams *imp, GameMtl *m); 
	~GameMtlDlg();

	// From ParamDlg:
	Class_ID				ClassID(void);
	void					SetThing(ReferenceTarget *m);
	ReferenceTarget*	GetThing(void) { return (ReferenceTarget*)TheMtl; }
	void					DeleteThis() { delete this;  }	
	void					SetTime(TimeValue t);
	void					ReloadDialog(void);
	void					ActivateDlg(BOOL onOff);

	void					Invalidate(void);
	void					Update_Display(void)	{ IParams->MtlChanged(); }

protected:

	void					Build_Dialog(void);
	
	BOOL					DisplacementMapProc(HWND hDlg, UINT message, WPARAM wParam, LPARAM lParam);
	BOOL					SurfaceTypeProc(HWND hDlg, UINT message, WPARAM wParam, LPARAM lParam);
	BOOL					PassCountProc(HWND hDlg, UINT message, WPARAM wParam, LPARAM lParam);
	void					Set_Pass_Count_Dialog(void);

	enum { MAX_PASSES = 4 };

	////////////////////////////////////////////////////////////////////////
	// Windows handles
	////////////////////////////////////////////////////////////////////////
	HWND					HwndEdit;		// window handle of the materials editor dialog
	HWND					HwndPassCount;	// Rollup pass count panel
	HWND					HwndSurfaceType;	// Rollup surface type panel
	HWND					HwndDisplacementMap;
	HPALETTE				HpalOld;

	GameMtlPassDlg *	PassDialog[MAX_PASSES];	

	////////////////////////////////////////////////////////////////////////
	// Material dialog interface
	////////////////////////////////////////////////////////////////////////
	IMtlParams *		IParams;			// interface to the material editor
	GameMtl *			TheMtl;			// current mtl being edited.
	
	////////////////////////////////////////////////////////////////////////
	// Member variables
	////////////////////////////////////////////////////////////////////////
	TimeValue			CurTime;
	int					IsActive;
	
	friend BOOL CALLBACK DisplacementMapDlgProc(HWND, UINT, WPARAM,LPARAM);
	friend BOOL CALLBACK SurfaceTypePanelDlgProc(HWND, UINT, WPARAM,LPARAM);
	friend BOOL CALLBACK PassCountPanelDlgProc(HWND, UINT, WPARAM,LPARAM);
	friend class GameMtl;
};


#endif