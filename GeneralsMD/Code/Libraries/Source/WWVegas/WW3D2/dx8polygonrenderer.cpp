#include "dx8polygonrenderer.h"
#include "dx8renderer.h"


// ----------------------------------------------------------------------------

DX8PolygonRendererClass::DX8PolygonRendererClass(
	unsigned index_count_,
	MeshModelClass* mmc_,
	DX8TextureCategoryClass* tex_cat,
	unsigned vertex_offset_,
	unsigned index_offset_,
	bool strip_,
	unsigned pass_)
	:
	mmc(mmc_), 
	texture_category(tex_cat),
	index_offset(index_offset_),
	vertex_offset(vertex_offset_),
	min_vertex_index(0),
	vertex_index_range(0),
	index_count(index_count_),
	strip(strip_),
	pass(pass_)
{
	WWASSERT(index_count);
	mmc->PolygonRendererList.Add_Tail(this);
}

DX8PolygonRendererClass::DX8PolygonRendererClass(const DX8PolygonRendererClass& src,MeshModelClass* mmc_)
	:
	mmc(mmc_), 
	texture_category(src.texture_category),
	index_offset(src.index_offset),
	vertex_offset(src.vertex_offset),
	min_vertex_index(src.min_vertex_index),
	vertex_index_range(src.vertex_index_range),
	index_count(src.index_count),
	strip(src.strip),
	pass(src.pass)
{
	mmc->PolygonRendererList.Add_Tail(this);
}

DX8PolygonRendererClass::~DX8PolygonRendererClass()
{
	if (texture_category) texture_category->Remove_Polygon_Renderer(this);
}

// ----------------------------------------------------------------------------

void DX8PolygonRendererClass::Log()
{
	StringClass work(true);

	work.Format("	%8d %8d %6d %6d %6d %s\n",
		index_count,
		index_count/3,
		index_offset,
		min_vertex_index,
		vertex_index_range,
		mmc->Get_Name());

/*	work.Format(
		"		Index count: %d (%d polys) i_offset: %d min_vi: %d vi_range: %d ident: %d (%s)\n",
		index_count,
		index_count/3,
		index_offset,
		min_vertex_index,
		vertex_index_range,
		mmc->ident,
		mmc->Get_Name());
*/	WWDEBUG_SAY((work));
}

