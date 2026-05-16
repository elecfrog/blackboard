import { createApp } from 'vue'
import { createRouter, createWebHashHistory } from 'vue-router'
import 'tdesign-vue-next/es/style/index.css'
import './styles.css'
import App from './App.vue'

const BoardView = () => import('./views/BoardView.vue')
const HomeView = () => import('./views/HomeView.vue')

function routeString(value: unknown) {
  return typeof value === 'string' ? value : undefined
}

function dashboardProps(
  section?: 'tickets' | 'taskGraphs' | 'inbox' | 'agents' | 'settings' | 'wiki',
  ticketView?: 'kanban' | 'graph' | 'list',
) {
  return (route: { params: Record<string, unknown>; query: Record<string, unknown> }) => {
    // wiki `:path(.*)*` comes in as string | string[] | undefined; normalize
    // to a POSIX-style relative path so the panel can look it up in the tree.
    const rawPath = route.params.path
    const wikiPath = Array.isArray(rawPath)
      ? rawPath.filter((p) => typeof p === 'string' && p.length > 0).join('/')
      : typeof rawPath === 'string'
        ? rawPath
        : undefined

    return {
      project: String(route.params.project),
      section,
      ticketView,
      id: routeString(route.params.id) ?? (section === 'tickets' ? routeString(route.query.preview) : undefined),
      path: section === 'wiki' ? wikiPath || undefined : routeString(route.params.path),
      focus: ticketView === 'graph' ? routeString(route.query.focus) : undefined,
      taskGraphScope: section === 'taskGraphs' ? routeString(route.params.scope) : undefined,
      taskGraphId: section === 'taskGraphs' ? routeString(route.params.graphId) : undefined,
      taskGraphMode: section === 'taskGraphs' ? routeString(route.params.mode) : undefined,
    }
  }
}

const router = createRouter({
  history: createWebHashHistory(),
  routes: [
    { path: '/', component: HomeView },
    { path: '/projects/:project', redirect: (to) => `/projects/${String(to.params.project)}/tickets` },
    { path: '/projects/:project/tickets', component: BoardView, props: dashboardProps('tickets', 'kanban') },
    { path: '/projects/:project/tickets/graph', component: BoardView, props: dashboardProps('tickets', 'graph') },
    { path: '/projects/:project/tickets/list', component: BoardView, props: dashboardProps('tickets', 'list') },
    { path: '/projects/:project/task-graphs', component: BoardView, props: dashboardProps('taskGraphs') },
    { path: '/projects/:project/task-graphs/:scope/:graphId/:mode?', component: BoardView, props: dashboardProps('taskGraphs') },
    { path: '/projects/:project/inbox', component: BoardView, props: dashboardProps('inbox') },
    { path: '/projects/:project/agents', component: BoardView, props: dashboardProps('agents') },
    {
      path: '/projects/:project/graph',
      redirect: (to) => ({
        path: `/projects/${String(to.params.project)}/tickets/graph`,
        query: to.query,
      }),
    },
    { path: '/projects/:project/settings', component: BoardView, props: dashboardProps('settings') },
    { path: '/projects/:project/wiki', component: BoardView, props: dashboardProps('wiki') },
    {
      path: '/projects/:project/wiki/:path(.*)*',
      component: BoardView,
      props: dashboardProps('wiki'),
    },
    {
      path: '/projects/:project/tickets/:id',
      component: BoardView,
      props: dashboardProps('tickets'),
    },
    { path: '/settings', redirect: '/' },
    // Legacy path compatibility for bookmarks that predate the project layout.
    {
      path: '/ticket/:id',
      redirect: (to) => ({
        path: '/',
        query: { legacyTicket: String(to.params.id) },
      }),
    },
  ],
})

createApp(App).use(router).mount('#app')
