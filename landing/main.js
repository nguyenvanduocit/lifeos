const views = [
  ['today','Tổng quan','“Sáng nay mình nên bắt đầu từ đâu?”'],
  ['tasks','Công việc','“Thêm việc gọi bố vào tối nay giúp mình.”'],
  ['goals','Mục tiêu','“Mục tiêu nào còn việc chưa xong?”'],
  ['habits','Thói quen','“Tuần này mình còn thiếu mấy buổi chạy?”'],
  ['money','Chi tiêu','“Tháng này khoản nào đã vượt ngân sách?”'],
  ['health','Sức khỏe','“Giấc ngủ của mình tháng này thay đổi thế nào?”'],
  ['journal','Nhật ký','“Ghi vào nhật ký hôm nay là mình đã chạy 5 km.”'],
  ['crew','Đội trợ lý','“Nhờ Coach nhìn lại tuần tập luyện của mình.”']
]
const el = selector => document.querySelector(selector)
const dialog = el('dialog')
function select([key,title,copy]){
  el('#shot').src = '/media/'+key+'.webp'
  el('#shot').alt = 'Giao diện LifeOS · '+title
  el('#shot-link').href = el('#shot').src
  el('#shot-link').dataset.zoom = key
  el('#shot-name').textContent = title
  el('#shot-copy').textContent = copy
  for(const button of el('.choices').children) button.setAttribute('aria-pressed',String(button.dataset.key === key))
}
for(const view of views){
  const button = document.createElement('button')
  button.textContent = view[1]
  button.dataset.key = view[0]
  button.onclick = () => select(view)
  el('.choices').append(button)
}
select(views[4])
function zoom(key){
  const title = views.find(view => view[0] === key)[1]
  el('#large').src = '/media/'+key+'.webp'
  el('#large').alt = 'Giao diện LifeOS · '+title
  el('#original').href = el('#large').src
  el('#zoom-title').textContent = title
  dialog.showModal()
}
for(const link of document.querySelectorAll('[data-zoom]')) link.onclick = event => {event.preventDefault(); zoom(link.dataset.zoom)}
el('#expand').onclick = () => zoom(el('#shot-link').dataset.zoom)
el('#close').onclick = () => dialog.close()
dialog.onclick = event => {if(event.target === dialog) dialog.close()}

const examples = [
  { label: 'Ghi việc', key: 'tasks', action: 'Thêm công việc', answer: 'Mình đã thêm “Gọi bố” vào danh sách việc hôm nay.' },
  { label: 'Hỏi chi tiêu', key: 'money', action: 'Đọc giao dịch và đối chiếu ngân sách', answer: 'Ăn uống đã chạm 4,1 triệu đồng, cao hơn mức 4 triệu bạn đặt cho tháng này.' },
  { label: 'Viết nhật ký', key: 'journal', action: 'Ghi vào nhật ký hôm nay', answer: 'Đã ghi lại buổi chạy 5 km. Cuối tuần bạn có thể nhờ mình đọc lại những buổi tập đã ghi.' }
]
function showExample(example) {
  const view = views.find(([key]) => key === example.key)
  el('#request').textContent = view[2]
  el('#action').textContent = '↳ ' + example.action
  el('#answer').textContent = example.answer
  el('#result').textContent = 'Xem màn hình ' + view[1].toLocaleLowerCase('vi') + ' ↓'
  el('#result').onclick = () => select(view)
  for (const button of el('.examples').children) {
    button.setAttribute('aria-pressed', String(button.textContent === example.label))
  }
}
for (const example of examples) {
  const button = document.createElement('button')
  button.textContent = example.label
  button.onclick = () => showExample(example)
  el('.examples').append(button)
}
showExample(examples[0])
