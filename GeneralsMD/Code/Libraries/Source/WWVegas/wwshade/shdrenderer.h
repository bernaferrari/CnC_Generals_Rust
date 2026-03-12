#ifndef SHDRENDERER_H
#define SHDRENDERER_H

#include "multilist.h"
#include "refcount.h"
#include "shdinterface.h"

class ShdSubMeshClass;
class ShdMeshClass;
class RenderInfoClass;

class ShdRendererNodeClass;
class RendererListContainerClass;

typedef MultiListClass<ShdRendererNodeClass> ShdRendererNodeList;
typedef MultiListIterator<ShdRendererNodeClass> ShdRendererNodeListIterator;
typedef MultiListClass<RendererListContainerClass> RendererListContainerList;
typedef MultiListIterator<RendererListContainerClass> RendererListContainerIterator;

class RendererListContainerClass : public MultiListObjectClass, public RefCountClass
{
	int Pass;
	ShdRendererNodeList LinkedNodes;
	ShdRendererNodeList VisibleNodes;
public:
	RendererListContainerClass(int pass);
	virtual ~RendererListContainerClass();

	void Add_Visible_Node(ShdRendererNodeClass* node) {
		VisibleNodes.Add_Tail(node);
	}

	ShdRendererNodeList& Peek_Linked_Nodes() { return LinkedNodes; }

	void Flush();

	void Register_Renderer(ShdRendererNodeClass* node);
	void Unregister_All();

};

/**
** ShdRendererNode
** RendererNodeClass acts as a link between ShdMeshClass, ShdSubMeshClass and the
** rendering system. Rendering API specific implementations are expected.
*/
class ShdRendererNodeClass : public MultiListObjectClass, public RefCountClass
{
	RendererListContainerClass* RendererListContainer[SHD_MAX_PASSES];
	int MaxPass;
public:
	ShdRendererNodeClass() : MaxPass(0)
	{
		for (int a=0;a<SHD_MAX_PASSES;++a) {
			RendererListContainer[a]=0;
		}
	}
	virtual ~ShdRendererNodeClass()
	{
		for (int a=0;a<SHD_MAX_PASSES;++a) {
			REF_PTR_RELEASE(RendererListContainer[a]);
		}
	}

	virtual void Render(RenderInfoClass& rinfo)=0;
	virtual void Flush(int pass)=0;
	virtual void Apply_Shared_Shader_Settings(ShdRendererNodeClass* prev_node, int pass)=0;

	virtual bool Greater_Than(const ShdRendererNodeClass&, int pass) const =0;

	void Connect()
	{
		for (int pass=0;pass<=MaxPass;++pass) {
			WWASSERT(RendererListContainer[pass]);
			if (RendererListContainer[pass]) RendererListContainer[pass]->Add_Visible_Node(this);
		}
	}

	void Set_Renderer_List_Container(RendererListContainerClass* cont,int pass)
	{
		REF_PTR_SET(RendererListContainer[pass],cont);
		if (cont) MaxPass=max(MaxPass,pass);
	}
	RendererListContainerClass* Peek_Renderer_List_Container(int pass) { return RendererListContainer[pass]; }
};


/**
** ShdRenderer
*/
class ShdRendererClass
{
	static ShdRendererClass* ShdRenderer;

protected:
	ShdRendererClass();
	virtual ~ShdRendererClass();
public:	

	/////////////////////////////////////////////////////////////////////////////////////
	// Init must be called before any objects can be rendered and Release() must be called
	// at the end to release all references to the rendering API. Working through a static
	// pointer instead of calling static functions will later allow us to switch to another
	// rendering API implementation of the renderer.
	/////////////////////////////////////////////////////////////////////////////////////
	static void Init();
	static void Shutdown();

	static void Init_Shaders();
	static void Shutdown_Shaders();

	static ShdRendererClass* Peek_Instance() { return ShdRenderer; }

	/////////////////////////////////////////////////////////////////////////////////////
	// Register initialized a mesh for rendering.
	/////////////////////////////////////////////////////////////////////////////////////
	virtual ShdRendererNodeClass* Register_Mesh(ShdMeshClass* mesh,ShdSubMeshClass* sub_mesh)=0;

	/////////////////////////////////////////////////////////////////////////////////////
	// Flush all linked meshes to the rendering system
	/////////////////////////////////////////////////////////////////////////////////////
	virtual void Flush()=0;
};


/**
** ShdDX8Renderer
*/
class ShdDX8RendererClass : public ShdRendererClass
{
public:
	ShdDX8RendererClass();
	virtual ~ShdDX8RendererClass();

	/////////////////////////////////////////////////////////////////////////////////////
	// Register initialized a mesh for rendering. Note, these will need to be virtual
	// if new types of renderers are created.
	/////////////////////////////////////////////////////////////////////////////////////
	virtual ShdRendererNodeClass* Register_Mesh(ShdMeshClass* mesh,ShdSubMeshClass* sub_mesh);

	/////////////////////////////////////////////////////////////////////////////////////
	// Flush all linked meshes to the rendering system
	/////////////////////////////////////////////////////////////////////////////////////
	virtual void Flush();

	class MeshContainerClass;
private:
	MeshContainerClass** MeshCategories;

};

#endif //SHDRENDERER_H
