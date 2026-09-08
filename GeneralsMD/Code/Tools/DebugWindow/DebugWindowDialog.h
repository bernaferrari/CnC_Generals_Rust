
#include "Resource.h"
#include <map>			// for std::pair
#include <string>		// for std::string
#include <vector>		// for std::vector

typedef std::pair<std::string, std::string>	PairString;
typedef std::vector<PairString>				VecPairString;
typedef std::vector<std::string>			VecString;

typedef std::vector<PairString>::iterator	VecPairStringIt;
typedef std::vector<std::string>::iterator	VecStringIt;


class DebugWindowDialog : public CDialog
{
	public:
		enum {IDD = IDD_DebugWindow};
		DebugWindowDialog(UINT nIDTemplate = DebugWindowDialog::IDD, CWnd* pParentWnd = NULL);
		
		bool CanProceed(void);
		bool RunAppFast(void);
		void AppendMessage(const std::string& messageToAppend);
		void AdjustVariable(const std::string& varName, const std::string& varValue);
		void SetFrameNumber(int frameNumber);
		HWND GetMainWndHWND(void);
		void ForcePause(void);
		void ForceContinue(void);

	// This var shouldn't be here, but honsestly...
	protected:
		HWND					mMainWndHWND;
		int						mNumberOfStepsAllowed;		/// -1 means go forever, 0 means stop now, a positive number will be decremented to 0, 1 frame at a time
		std::string		mVariablesString;
		std::string		mMessagesString;
		CString				mVariablesDisplayString;	// For double buffering
		CString				mMessagesDisplayString;		// For double buffering
		std::string		mFrameNumber;
		bool					mStepping;
		bool					mRunFast;



		VecPairString	mVariables;
		VecString		mMessages;

		void _RebuildVarsString(void);
		void _RebuildMesgString(void);
		void _UpdatePauseButton(void);

	protected:
		afx_msg int OnCreate(LPCREATESTRUCT lpCreateStruct);
		afx_msg void OnSize(UINT nType, int cx, int cy);
		afx_msg void OnPause();
		afx_msg void OnStep();
		afx_msg void OnRunFast();
		afx_msg void OnStepTen();
		afx_msg void OnClearWindows();
		afx_msg void OnClose();
		DECLARE_MESSAGE_MAP()

	

};