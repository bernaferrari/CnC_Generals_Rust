#pragma once

// Declared extern C to prevent name mangling, which makes life very unhappy
extern "C" {
	// Called to create the dialog
	void __declspec(dllexport) CreateDebugDialog(void);
	
	// Called to (not surprisingly) destroy the dialog (and free the resources)
	void __declspec(dllexport) DestroyDebugDialog(void);

	// Call this each frame to determine whether to continue or not
	bool __declspec(dllexport) CanAppContinue(void);

	// Call this to force the app to continue. (Unpause if necessary.)
	void __declspec(dllexport) ForceAppContinue(void);

	// Call this to tell the app to run really fast
	bool __declspec(dllexport) RunAppFast(void);

	// Call this to add a message to the script window
	void __declspec(dllexport) AppendMessage(const char* messageToPass);

	// Call this to set the frame number of the app
	void __declspec(dllexport) SetFrameNumber(int frameNumber);

	// Call this to add a message, and simulate pressing Pause immediately after
	void __declspec(dllexport) AppendMessageAndPause(const char* messageToPass);

	// Call this to add or update a variable value 
	void __declspec(dllexport) AdjustVariable(const char* variable, const char* value);

	// Call this to add or update a variable value, and simulate pressing Pause immediately after
	void __declspec(dllexport) AdjustVariableAndPause(const char* variable, const char* value);
}